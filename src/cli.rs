use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "SimulateTraffic")]
#[command(about = "A tool to simulate network traffic")]
pub struct Cli {
    #[arg(value_parser = check_if_file_is_a_json,help="Path to the JSON configuration file")]
    pub json_path: PathBuf,
    #[arg(value_parser=check_hdf5_file_extension,short,long)]
    pub output_file_hdf5: Option<PathBuf>,
    #[arg(short,long,default_value_t=default_thread_count())]
    pub threads: usize,
    #[arg(short, long, default_value_t = false)]
    pub force: bool,
}

fn default_thread_count() -> usize {
    let default_percentage = 0.5;

    let logical_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    (logical_cores as f64 * default_percentage).round() as usize
}

fn check_if_file_is_a_json(s: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(s);
    if !path.exists() {
        return Err(format!("The path '{}' does not exist.", s));
    }
    if !path.is_file() {
        return Err(format!("'{}' is a directory, not a file.", s));
    }

    let has_json_ext = path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("json"))
        .unwrap_or(false);

    if !has_json_ext {
        return Err(format!("The file '{}' must have a .json extension.", s));
    }

    Ok(path)
}

fn check_hdf5_file_extension(s: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(s);
    let has_hdf5_ext = path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("hdf5"))
        .unwrap_or(false);

    if !has_hdf5_ext {
        return Err(format!("The file '{}' must have a .hdf5 extension.", s));
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_cli_default_thread_count() {
        let count = default_thread_count();
        assert!(count > 0);
    }

    #[test]
    fn test_cli_validate_json_file_success() {
        let mut path = env::temp_dir();
        path.push(format!("test_{}.json", uuid::Uuid::new_v4()));

        let mut file = File::create(&path).unwrap();
        writeln!(file, "{{}}").unwrap();

        let result = check_if_file_is_a_json(path.to_str().unwrap());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), path);

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_cli_validate_json_file_not_exist() {
        let mut path = env::temp_dir();
        path.push("non_existent_file.json");
        if path.exists() {
            std::fs::remove_file(&path).ok();
        }
        let result = check_if_file_is_a_json(path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn test_cli_validate_json_file_is_directory() {
        let mut path = env::temp_dir();
        path.push(format!("test_dir_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();

        let result = check_if_file_is_a_json(path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("is a directory"));

        std::fs::remove_dir(path).unwrap();
    }

    #[test]
    fn test_cli_validate_json_file_wrong_extension() {
        let mut path = env::temp_dir();
        path.push(format!("test_{}.txt", uuid::Uuid::new_v4()));

        let mut file = File::create(&path).unwrap();
        writeln!(file, "content").unwrap();

        let result = check_if_file_is_a_json(path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must have a .json extension"));

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_cli_parsing() {
        let mut path = env::temp_dir();
        path.push(format!("cli_test_{}.json", uuid::Uuid::new_v4()));
        File::create(&path).unwrap();

        let path_str = path.to_str().unwrap();

        let args = vec!["program_name", path_str];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.json_path, path);

        let args = vec!["program_name", path_str, "--threads", "4"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.threads, 4);

        std::fs::remove_file(path).unwrap();
    }
}
