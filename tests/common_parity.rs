#![allow(missing_docs)]

use pkg_rust::{
    PathStyle, inside_snapshot, normalize_path_text, remove_uplevels, retrieve_denominator,
    snapshotify, strip_snapshot, substitute_denominator,
};

fn substitute_many(files: &[&str]) -> Vec<String> {
    let denominator = retrieve_denominator(files, PathStyle::Posix);
    files
        .iter()
        .map(|file| substitute_denominator(file, denominator, PathStyle::Posix))
        .collect()
}

fn substitute_many_with_style(files: &[&str], style: PathStyle) -> Vec<String> {
    let denominator = retrieve_denominator(files, style);
    files
        .iter()
        .map(|file| substitute_denominator(file, denominator, style))
        .collect()
}

#[test]
fn posix_normalize_matches_test_48_common() {
    assert_eq!(normalize_path_text("/", PathStyle::Posix), "/");
    assert_eq!(normalize_path_text("//", PathStyle::Posix), "/");
    assert_eq!(
        normalize_path_text("/snapshot", PathStyle::Posix),
        "/snapshot"
    );
    assert_eq!(
        normalize_path_text("/snapshoter", PathStyle::Posix),
        "/snapshoter"
    );
    assert_eq!(
        normalize_path_text("/snapshot/", PathStyle::Posix),
        "/snapshot"
    );
    assert_eq!(
        normalize_path_text("/snapshoter/", PathStyle::Posix),
        "/snapshoter"
    );
    assert_eq!(
        normalize_path_text("/snapshot//foo", PathStyle::Posix),
        "/snapshot/foo"
    );
    assert_eq!(
        normalize_path_text("/snapshot//foo//bar/\\//", PathStyle::Posix),
        "/snapshot/foo/bar"
    );
}

#[test]
fn posix_snapshot_detection_matches_test_48_common() {
    assert!(!inside_snapshot("", PathStyle::Posix));
    assert!(!inside_snapshot("/", PathStyle::Posix));
    assert!(!inside_snapshot("/foo", PathStyle::Posix));
    assert!(!inside_snapshot("/foo/snapshot", PathStyle::Posix));
    assert!(inside_snapshot("/snapshot", PathStyle::Posix));
    assert!(!inside_snapshot("/snapshoter", PathStyle::Posix));
    assert!(inside_snapshot("/snapshot/", PathStyle::Posix));
    assert!(!inside_snapshot("/snapshoter/", PathStyle::Posix));
    assert!(inside_snapshot("/snapshot//", PathStyle::Posix));
    assert!(inside_snapshot("/snapshot/foo", PathStyle::Posix));
    assert!(!inside_snapshot("/snapshoter/foo", PathStyle::Posix));
}

#[test]
fn posix_strip_snapshot_matches_test_48_common() {
    assert_eq!(strip_snapshot("/", PathStyle::Posix), "/");
    assert_eq!(strip_snapshot("//", PathStyle::Posix), "//");
    assert_eq!(strip_snapshot("/snapshot", PathStyle::Posix), "/**/");
    assert_eq!(
        strip_snapshot("/snapshoter", PathStyle::Posix),
        "/snapshoter"
    );
    assert_eq!(strip_snapshot("/snapshot/", PathStyle::Posix), "/**/");
    assert_eq!(
        strip_snapshot("/snapshoter/", PathStyle::Posix),
        "/snapshoter/"
    );
    assert_eq!(
        strip_snapshot("/snapshot//foo", PathStyle::Posix),
        "/**/foo"
    );
    assert_eq!(
        strip_snapshot("/snapshot//foo//bar/\\//", PathStyle::Posix),
        "/**/foo/bar"
    );
}

#[test]
fn posix_snapshotify_and_uplevels_match_test_48_common() {
    assert_eq!(snapshotify("/", PathStyle::Posix), "/snapshot");
    assert_eq!(snapshotify("/foo", PathStyle::Posix), "/snapshot/foo");
    assert_eq!(
        snapshotify("/foo/bar", PathStyle::Posix),
        "/snapshot/foo/bar"
    );

    assert_eq!(remove_uplevels("../foo", PathStyle::Posix), "foo");
    assert_eq!(remove_uplevels("../../foo", PathStyle::Posix), "foo");
    assert_eq!(remove_uplevels("./foo", PathStyle::Posix), "./foo");
    assert_eq!(remove_uplevels(".", PathStyle::Posix), ".");
    assert_eq!(remove_uplevels("..", PathStyle::Posix), ".");
    assert_eq!(remove_uplevels("../..", PathStyle::Posix), ".");
}

#[test]
fn posix_denominator_substitution_matches_test_48_common() {
    assert_eq!(
        substitute_many(&["/long/haired/freaky/people", "/long/haired/aliens"]),
        vec!["/freaky/people", "/aliens"]
    );

    assert_eq!(
        substitute_many(&["/long/haired/freaky/people", "/long/hyphen/sign"]),
        vec!["/haired/freaky/people", "/hyphen/sign"]
    );
}

#[test]
fn windows_normalize_matches_test_48_common() {
    assert_eq!(normalize_path_text("c:", PathStyle::Windows), "c:");
    assert_eq!(normalize_path_text("c:\\", PathStyle::Windows), "C:\\");
    assert_eq!(normalize_path_text("c:\\\\", PathStyle::Windows), "C:\\");
    assert_eq!(
        normalize_path_text("c:\\snapshot", PathStyle::Windows),
        "C:\\snapshot"
    );
    assert_eq!(
        normalize_path_text("c:\\snapshoter", PathStyle::Windows),
        "C:\\snapshoter"
    );
    assert_eq!(
        normalize_path_text("c:\\snapshot\\", PathStyle::Windows),
        "C:\\snapshot"
    );
    assert_eq!(
        normalize_path_text("c:\\snapshoter\\", PathStyle::Windows),
        "C:\\snapshoter"
    );
    assert_eq!(
        normalize_path_text("c:\\snapshot\\\\foo", PathStyle::Windows),
        "C:\\snapshot\\foo"
    );
    assert_eq!(
        normalize_path_text("c:\\snapshot\\\\foo\\\\bar\\/\\\\", PathStyle::Windows),
        "C:\\snapshot\\foo\\bar"
    );
}

#[test]
fn windows_snapshot_detection_matches_test_48_common() {
    assert!(!inside_snapshot("c:", PathStyle::Windows));
    assert!(!inside_snapshot("c:\\", PathStyle::Windows));
    assert!(!inside_snapshot("c:\\foo", PathStyle::Windows));
    assert!(!inside_snapshot("c:\\foo\\snapshot", PathStyle::Windows));
    assert!(inside_snapshot("c:\\snapshot", PathStyle::Windows));
    assert!(!inside_snapshot("c:\\snapshoter", PathStyle::Windows));
    assert!(inside_snapshot("c:\\snapshot\\", PathStyle::Windows));
    assert!(!inside_snapshot("c:\\snapshoter\\", PathStyle::Windows));
    assert!(inside_snapshot("c:\\snapshot\\\\", PathStyle::Windows));
    assert!(inside_snapshot("c:\\snapshot\\foo", PathStyle::Windows));
    assert!(!inside_snapshot("c:\\snapshoter\\foo", PathStyle::Windows));
}

#[test]
fn windows_strip_snapshot_matches_test_48_common() {
    assert_eq!(strip_snapshot("c:\\", PathStyle::Windows), "c:\\");
    assert_eq!(strip_snapshot("c:\\\\", PathStyle::Windows), "c:\\\\");
    assert_eq!(
        strip_snapshot("c:\\snapshot", PathStyle::Windows),
        "C:\\**\\"
    );
    assert_eq!(
        strip_snapshot("c:\\snapshoter", PathStyle::Windows),
        "c:\\snapshoter"
    );
    assert_eq!(
        strip_snapshot("c:\\snapshot\\", PathStyle::Windows),
        "C:\\**\\"
    );
    assert_eq!(
        strip_snapshot("c:\\snapshoter\\", PathStyle::Windows),
        "c:\\snapshoter\\"
    );
    assert_eq!(
        strip_snapshot("c:\\snapshot\\\\foo", PathStyle::Windows),
        "C:\\**\\foo"
    );
    assert_eq!(
        strip_snapshot("c:\\snapshot\\\\foo\\\\bar\\/\\\\", PathStyle::Windows),
        "C:\\**\\foo\\bar"
    );
}

#[test]
fn windows_snapshotify_and_uplevels_match_test_48_common() {
    assert_eq!(snapshotify("C:\\", PathStyle::Windows), "C:\\snapshot");
    assert_eq!(
        snapshotify("C:\\foo", PathStyle::Windows),
        "C:\\snapshot\\foo"
    );
    assert_eq!(
        snapshotify("C:\\foo\\bar", PathStyle::Windows),
        "C:\\snapshot\\foo\\bar"
    );

    assert_eq!(remove_uplevels("..\\foo", PathStyle::Windows), "foo");
    assert_eq!(remove_uplevels("..\\..\\foo", PathStyle::Windows), "foo");
    assert_eq!(remove_uplevels(".\\foo", PathStyle::Windows), ".\\foo");
    assert_eq!(remove_uplevels(".", PathStyle::Windows), ".");
    assert_eq!(remove_uplevels("..", PathStyle::Windows), ".");
    assert_eq!(remove_uplevels("..\\..", PathStyle::Windows), ".");
}

#[test]
fn windows_denominator_substitution_matches_test_48_common() {
    assert_eq!(
        substitute_many_with_style(
            &[
                "C:\\long\\haired\\freaky\\people",
                "C:\\long\\haired\\aliens",
            ],
            PathStyle::Windows
        ),
        vec!["C:\\freaky\\people", "C:\\aliens"]
    );

    assert_eq!(
        substitute_many_with_style(
            &["C:\\long\\haired\\freaky\\people", "C:\\long\\hyphen\\sign",],
            PathStyle::Windows
        ),
        vec!["C:\\haired\\freaky\\people", "C:\\hyphen\\sign"]
    );

    assert_eq!(
        substitute_many_with_style(
            &["C:\\long\\haired\\freaky\\people", "D:\\long\\hyphen\\sign",],
            PathStyle::Windows
        ),
        vec!["C:\\long\\haired\\freaky\\people", "D:\\long\\hyphen\\sign"]
    );
}
