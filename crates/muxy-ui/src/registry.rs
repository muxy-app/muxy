use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegSetKeyValueW};
use windows::core::HSTRING;

pub type Value = (String, &'static str, String);

pub fn set_user_values(values: Vec<Value>) -> Result<(), String> {
    for (key, name, value) in values {
        let key = HSTRING::from(key);
        let data: Vec<u16> = value.encode_utf16().chain([0]).collect();
        let bytes = u32::try_from(data.len() * 2).map_err(|error| error.to_string())?;
        let result = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &key,
                &HSTRING::from(name),
                REG_SZ.0,
                Some(data.as_ptr().cast()),
                bytes,
            )
        };
        if result != ERROR_SUCCESS {
            return Err(windows::core::Error::from(result.to_hresult()).message());
        }
    }
    Ok(())
}

pub fn register_url_scheme() -> Result<(), String> {
    let channel = muxy_core::release::Channel::current();
    let executable = muxy_core::executable::current_path().map_err(|error| error.to_string())?;
    set_user_values(url_scheme(
        channel.url_scheme(),
        channel.app_name(),
        &executable.to_string_lossy(),
    ))
}

fn url_scheme(scheme: &str, name: &str, executable: &str) -> Vec<Value> {
    let key = format!(r"Software\Classes\{scheme}");
    vec![
        (key.clone(), "", format!("URL:{name}")),
        (key.clone(), "URL Protocol", String::new()),
        (
            format!(r"{key}\shell\open\command"),
            "",
            format!("\"{executable}\" \"%1\""),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_launch_this_copy_of_the_app_with_the_link() {
        assert_eq!(
            url_scheme("muxy-beta", "Muxy Beta", r"C:\Muxy Beta\muxy-app.exe"),
            [
                (
                    r"Software\Classes\muxy-beta".to_owned(),
                    "",
                    "URL:Muxy Beta".to_owned()
                ),
                (
                    r"Software\Classes\muxy-beta".to_owned(),
                    "URL Protocol",
                    String::new()
                ),
                (
                    r"Software\Classes\muxy-beta\shell\open\command".to_owned(),
                    "",
                    r#""C:\Muxy Beta\muxy-app.exe" "%1""#.to_owned()
                ),
            ]
        );
    }
}
