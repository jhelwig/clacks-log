use super::*;

#[test]
fn endpoint_directory_rejects_other_users_broad_permissions_and_symlinks() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))?;
    let uid = directory.path().metadata()?.uid();
    verify_directory(directory.path(), uid)?;
    assert!(verify_directory(directory.path(), uid.wrapping_add(1)).is_err());
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o777))?;
    assert!(verify_directory(directory.path(), uid).is_err());
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))?;
    let parent = tempfile::tempdir()?;
    let alias = parent.path().join("alias");
    std::os::unix::fs::symlink(directory.path(), &alias)?;
    assert!(verify_directory(&alias, uid).is_err());
    Ok(())
}
