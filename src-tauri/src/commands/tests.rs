use super::targets::is_uri_or_file_path;

#[test]
fn recognizes_urls_and_file_paths() {
    assert!(is_uri_or_file_path("https://example.com"));
    assert!(is_uri_or_file_path("C:\\Temp\\file.txt"));
    assert!(is_uri_or_file_path("/tmp/file.txt"));
    assert!(!is_uri_or_file_path("javascript://alert(1)"));
    assert!(!is_uri_or_file_path("search these words"));
}
