use std::path::PathBuf;
use url::Url;

// Create a file URI
pub fn path_to_uri(path: PathBuf) -> String {
    let path_str = path.display().to_string();
    // Eventually this will be away to keep track of multiple devices and searching across
    // them.
    let host = "localhost";

    let mut new_url = Url::parse("file://").expect("Base URI");
    let _ = new_url.set_host(Some(host));
    // Fixes issues handling windows drive letters
    let path_str = path_str.replace(':', "%3A");
    // Fixes an issue where DirEntry adds too many escapes.
    let path_str = path_str.replace("\\\\", "\\");
    new_url.set_path(&path_str);
    new_url.to_string()
}

#[cfg(test)]
mod test {
    use super::path_to_uri;
    use std::path::Path;
    use url::Url;

    #[test]
    #[cfg(target_os = "windows")]
    fn test_path_to_uri() {
        let test_folder = Path::new("C:\\tmp\\path_to_uri");
        std::fs::create_dir_all(test_folder).expect("Unable to create test dir");

        let test_path = test_folder.join("test.txt");
        let uri = path_to_uri(test_path.to_path_buf());

        assert_eq!(uri, "file://localhost/C%3A/tmp/path_to_uri/test.txt");
        let url = Url::parse(&uri).unwrap();
        let file_path = url.to_file_path().unwrap();
        assert_eq!(file_path, test_path);
    }
}
