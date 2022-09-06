use std::fs::File;
use std::io::Result;
use std::path::PathBuf;
use std::time::SystemTime;
use tar::Builder;

// Utility method used to create a tar file from the specified directory.
// The generated tar file will have the same name as the directory with
// the utc timestamp and the extension "old" append to it.
pub fn backup_dir(dir: &PathBuf) -> Result<()> {
    let prefix = dir.file_name().unwrap();
    let time_str = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string();
    // Name of output tar file
    let output_file = format!("{}.{}.{}", prefix.to_str().unwrap(), time_str, "old");

    let logging_str = output_file.clone();

    // Full path to output tar file
    let backup_path = dir.parent().unwrap().join(output_file);

    let file_result = File::create(backup_path);
    if let Err(e) = file_result {
        println!(
            "Error generating backup file {:?} for directory {:?}, Error: {:?}",
            logging_str, dir, e
        );
        return Err(e);
    }

    let file = file_result.unwrap();
    let mut tar_builder = Builder::new(file);

    let tar_response = tar_builder.append_dir_all(".", dir);
    if let Err(e) = tar_response {
        println!("Error adding files to tar, {:?}", e);
        return Err(e);
    }
    Ok(())
}

// Utility method replaces the destination directory with the contents of
// the source directory. Linux/MacOS uses the rename command while windows
// uses a manual process (Gooooo Windows!!! :( ).
// Windows Process:
// 1. Delete destination folder and all contents
// 2. Recreate empty destination folder
// 3. Copy each file from source folder into destination folder
//    3a. Note directories are not currently processed
// 4. Delete source directory and all contents
//
// Linux Process:
// 1. Delete destination directory and all contents
// 2. Rename source directory to name used by destination directory
pub fn replace_dir(source: &PathBuf, dest: &PathBuf) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        // Step 1 delete destination
        std::fs::remove_dir_all(dest)?;

        // Step 2 recreate dest directory
        std::fs::create_dir(dest)?;
        
        // Step 3 Copy files from source to destination
        for entry in std::fs::read_dir(source)? {
            if let Err(e) = entry {
                println!("Error accessing index File {:?} ", e);
                return Err(e);
            }

            let path = entry?.path();
            if path.is_file() {
                match path.file_name() {
                    Some(filename) => {
                        let dest_path = dest.join(filename);
                        println!("  copy: {:?} -> {:?}", &path, &dest_path);
                        std::fs::copy(&path, &dest_path)?;
                    }
                    None => {
                        println!("failed: {:?}", path);
                    }
                }
            }
        }

        // Step 4 delete source
        std::fs::remove_dir_all(source)?;

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Step 1 delete destination
        if let Err(e) = std::fs::remove_dir_all(dest) {
            return Err(e);
        }

        // Step 2 Rename
        std::fs::rename(source, dest)
    }
}
