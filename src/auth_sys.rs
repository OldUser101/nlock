#[cfg(not(target_os = "openbsd"))]
mod pam_backend {
    use std::{
        ffi::{CStr, CString},
        os::raw::{c_char, c_int, c_void},
    };

    use anyhow::{Result, anyhow};
    use nix::libc::{self, size_t};
    use tracing::{debug, warn};
    use zeroize::Zeroizing;

    use crate::auth_sys::get_current_username;

    const PAM_PROMPT_ECHO_OFF: c_int = 1;
    const PAM_PROMPT_ECHO_ON: c_int = 2;
    const PAM_ERROR_MSG: c_int = 3;
    const PAM_TEXT_INFO: c_int = 4;

    const PAM_SUCCESS: c_int = 0;
    const PAM_BUF_ERR: c_int = 5;
    const PAM_CONV_ERR: c_int = 19;

    #[repr(C)]
    struct pam_handle_t {
        _private: [u8; 0],
    }

    #[repr(C)]
    struct pam_message {
        msg_style: c_int,
        msg: *const c_char,
    }

    #[repr(C)]
    struct pam_response {
        resp: *mut c_char,
        resp_retcode: c_int,
    }

    #[repr(C)]
    struct pam_conv {
        conv: Option<
            unsafe extern "C" fn(
                num_msg: c_int,
                msg: *mut *const pam_message,
                resp: *mut *mut pam_response,
                appdata_ptr: *mut c_void,
            ) -> c_int,
        >,
        appdata_ptr: *mut c_void,
    }

    unsafe extern "C" {
        fn pam_acct_mgmt(pamh: *mut pam_handle_t, flags: c_int) -> c_int;
        fn pam_authenticate(pamh: *mut pam_handle_t, flags: c_int) -> c_int;
        fn pam_end(pamh: *mut pam_handle_t, pam_status: c_int) -> c_int;
        fn pam_start(
            service_name: *const c_char,
            user: *const c_char,
            pam_conversation: *const pam_conv,
            pamh: *mut *mut pam_handle_t,
        ) -> c_int;
        fn pam_strerror(pamh: *const pam_handle_t, errnum: c_int) -> *const c_char;
    }

    struct ConvState {
        password: Option<Zeroizing<String>>,
    }

    pub struct AuthClient {
        user: String,
        conv: pam_conv,
        pamh: *mut pam_handle_t,
        pamres: c_int,
        conv_state: Box<ConvState>,
    }

    impl AuthClient {
        pub fn new<S>(service: S) -> Result<Self>
        where
            S: AsRef<str>,
        {
            debug!("using PAM backend");

            let user = get_current_username()?;

            debug!("setting up auth for '{}'", user);

            let mut conv_state = Box::new(ConvState { password: None });

            let appdata_ptr = conv_state.as_mut() as *mut ConvState as *mut c_void;

            let mut s = Self {
                user,
                conv: pam_conv {
                    conv: Some(Self::handle_conversation),
                    appdata_ptr,
                },
                pamh: std::ptr::null_mut(),
                pamres: PAM_SUCCESS,
                conv_state,
            };

            s.start(service)?;

            Ok(s)
        }

        fn handle_pam_error(&mut self) -> Result<()> {
            if self.pamres == PAM_SUCCESS {
                return Ok(());
            }

            unsafe {
                let err = pam_strerror(self.pamh as *const pam_handle_t, self.pamres);
                let err = CStr::from_ptr(err).to_string_lossy().to_string();
                self.clear_password(); // always invalidate password if something fails
                Err(anyhow!("pam error: {err}"))
            }
        }

        fn start<S>(&mut self, service: S) -> Result<()>
        where
            S: AsRef<str>,
        {
            unsafe {
                let user = CString::new(self.user.as_str())?;
                let service = CString::new(service.as_ref())?;
                self.pamres = pam_start(
                    service.as_ptr(),
                    user.as_ptr(),
                    &self.conv as *const pam_conv,
                    &mut self.pamh as *mut *mut pam_handle_t,
                );
                self.handle_pam_error()?;

                debug!("started new pam conversation");

                Ok(())
            }
        }

        pub fn set_password(&mut self, password: Zeroizing<String>) {
            self.conv_state.password = Some(password);
        }

        pub fn clear_password(&mut self) {
            self.conv_state.password = None;
        }

        pub fn authenticate(&mut self) -> Result<()> {
            if self.conv_state.password.is_none() {
                return Err(anyhow!("password has not been set"));
            }

            unsafe {
                self.pamres = pam_authenticate(self.pamh, 0);
                self.handle_pam_error()?;
                self.pamres = pam_acct_mgmt(self.pamh, 0);
                self.handle_pam_error()?;
            }

            self.clear_password();
            Ok(())
        }

        unsafe extern "C" fn handle_conversation(
            num_msg: c_int,
            msg: *mut *const pam_message,
            resp: *mut *mut pam_response,
            appdata_ptr: *mut c_void,
        ) -> c_int {
            if appdata_ptr.is_null() {
                return PAM_BUF_ERR;
            }
            if num_msg <= 0 || msg.is_null() || resp.is_null() {
                return PAM_CONV_ERR;
            }

            let conv_state = unsafe { &mut *(appdata_ptr as *mut ConvState) };

            let responses = unsafe {
                libc::calloc(
                    num_msg as size_t,
                    std::mem::size_of::<pam_response>() as size_t,
                ) as *mut pam_response
            };
            if responses.is_null() {
                return PAM_BUF_ERR;
            }

            for i in 0..num_msg as usize {
                let message = unsafe { *msg.add(i) };

                if message.is_null() {
                    return PAM_CONV_ERR;
                }

                match unsafe { (*message).msg_style } {
                    PAM_PROMPT_ECHO_ON | PAM_PROMPT_ECHO_OFF => {
                        let Some(password) = &conv_state.password else {
                            continue;
                        };

                        // easier to zero a raw buffer than a cstring
                        let mut buf = Zeroizing::new(password.as_bytes().to_vec());
                        buf.push(0);

                        let dup = unsafe { libc::strdup(buf.as_ptr() as *const c_char) };
                        if dup.is_null() {
                            unsafe {
                                libc::free(responses as *mut c_void);
                            }
                            return PAM_BUF_ERR;
                        }

                        unsafe {
                            (*responses.add(i)).resp = dup;
                            (*responses.add(i)).resp_retcode = 0;
                        }

                        conv_state.password = None;
                    }

                    PAM_TEXT_INFO | PAM_ERROR_MSG => {}

                    _ => {
                        unsafe {
                            libc::free(responses as *mut c_void);
                        }
                        return PAM_CONV_ERR;
                    }
                }
            }

            unsafe {
                *resp = responses;
            }

            PAM_SUCCESS
        }
    }

    impl Drop for AuthClient {
        fn drop(&mut self) {
            unsafe {
                self.pamres = pam_end(self.pamh, self.pamres);
                if let Err(e) = self.handle_pam_error() {
                    warn!("pam_end failed: {e}");
                }
                debug!("pam conversation closed");
            }
        }
    }

    // # Safety
    // Assumed PAM handle can be safely moved across threads
    unsafe impl Send for AuthClient {}
}

#[cfg(target_os = "openbsd")]
mod bsdauth_backend {
    use std::os::raw::{c_char, c_int};

    use anyhow::{Result, anyhow};
    use nix::unistd::{Group, setgid};
    use tracing::debug;
    use zeroize::Zeroizing;

    use crate::auth_sys::get_current_username;

    unsafe extern "C" {
        fn auth_userokay(
            name: *mut c_char,
            style: *mut c_char,
            service: *mut c_char,
            password: *mut c_char,
        ) -> c_int;
    }

    pub struct AuthClient {
        user: String,
        service: String,
        password: Option<Zeroizing<String>>,
    }

    impl AuthClient {
        pub fn new<S>(service: S) -> Result<Self>
        where
            S: AsRef<str>,
        {
            debug!("using BSD Authentication backend");

            let user = get_current_username()?;

            debug!("setting up auth for '{}'", user);

            let Some(authg) = Group::from_name("auth")? else {
                return Err(anyhow!("'auth' group does not exist?"));
            };

            // ensure we are setgid auth first
            if let Err(e) = setgid(authg.gid) {
                return Err(anyhow!(
                    "nlock was compiled with BSD Authentication, but is not setgid auth: {e}"
                ));
            }

            Ok(Self {
                user,
                service: service.as_ref().to_string(),
                password: None,
            })
        }

        pub fn set_password(&mut self, password: Zeroizing<String>) {
            self.password = Some(password);
        }

        pub fn clear_password(&mut self) {
            self.password = None;
        }

        pub fn authenticate(&mut self) -> Result<()> {
            let Some(password) = self.password.take() else {
                return Err(anyhow!("password has not been set"));
            };

            let mut user = self.user.clone().into_bytes();
            user.push(0);
            let mut service = self.service.clone().into_bytes();
            service.push(0);
            let mut buf = Zeroizing::new(password.as_bytes().to_vec());
            buf.push(0);

            let res = unsafe {
                auth_userokay(
                    user.as_mut_ptr() as *mut c_char,
                    service.as_mut_ptr() as *mut c_char,
                    std::ptr::null_mut(),
                    buf.as_mut_ptr() as *mut c_char,
                )
            };

            // 0=fail, anything else=success
            if res == 0 {
                return Err(anyhow!("authentication failure"));
            }

            Ok(())
        }
    }
}

pub fn get_current_username() -> anyhow::Result<String> {
    let uid = nix::unistd::getuid();
    let Some(user) = nix::unistd::User::from_uid(uid)? else {
        return Err(anyhow::anyhow!("who the f**k are you? (uid {:?})", uid));
    };
    Ok(user.name)
}

#[cfg(target_os = "openbsd")]
pub use bsdauth_backend::*;
#[cfg(not(target_os = "openbsd"))]
pub use pam_backend::*;
