//! One-time ImageIO conversion of selected artwork into a multi-image ICO.
use std::{env, fs, io, path::PathBuf, process::Command};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = PathBuf::from(env::args_os().nth(1).ok_or("expected source PNG")?);
    let output = PathBuf::from(env::args_os().nth(2).ok_or("expected output ICO")?);
    let scratch = env::temp_dir().join(format!("clacks-icon-{}", std::process::id()));
    fs::create_dir(&scratch)?;
    let sizes = [16_u16, 24, 32, 48, 64, 128, 256];
    let mut images = Vec::new();
    for size in sizes {
        let path = scratch.join(format!("{size}.png"));
        let status = Command::new("sips")
            .args(["-z", &size.to_string(), &size.to_string()])
            .arg(&source)
            .arg("--out")
            .arg(&path)
            .output()?;
        if !status.status.success() {
            return Err(io::Error::other("sips conversion failed").into());
        }
        images.push(fs::read(path)?);
    }
    let mut bytes = vec![0, 0, 1, 0, 7, 0];
    let mut offset = 6_u32 + 16 * 7;
    for (size, image) in sizes.into_iter().zip(&images) {
        let dimension = if size == 256 { 0 } else { u8::try_from(size)? };
        bytes.extend_from_slice(&[dimension, dimension, 0, 0, 1, 0, 32, 0]);
        let length = u32::try_from(image.len())?;
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend_from_slice(&offset.to_le_bytes());
        offset = offset.checked_add(length).ok_or("ICO offset overflow")?;
    }
    for image in images {
        bytes.extend_from_slice(&image);
    }
    fs::write(output, bytes)?;
    fs::remove_dir_all(scratch)?;
    Ok(())
}
