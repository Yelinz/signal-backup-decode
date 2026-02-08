// imports
use anyhow::anyhow;
use anyhow::Context;
use clap::Parser;
use std::io::BufRead;

#[derive(Parser)]
#[command(name = clap::crate_name!())]
#[command(version = clap::crate_version!())]
#[command(about = clap::crate_description!())]
#[command(author = clap::crate_authors!())]
struct Args {
	/// Sets the input file to use
	#[arg(value_name = "INPUT", required = true)]
	input_file: std::path::PathBuf,

	/// Directory to save output to. If not given, input file directory is used
	#[arg(short = 'o', long = "output-path", value_name = "FOLDER")]
	output_path: Option<std::path::PathBuf>,

	/// Output type, either RAW, CSV or NONE
	#[arg(short = 't', long = "output-type", value_name = "TYPE")]
	output_type: Option<String>,

	/// Verbosity level, either DEBUG, INFO, WARN, or ERROR
	#[arg(short = 'v', long = "verbosity", value_name = "LEVEL")]
	log_level: Option<String>,

	/// Overwrite existing output files
	#[arg(short = 'f', long = "force")]
	force_overwrite: bool,

	/// Do not verify the HMAC of each frame in the backup
	#[arg(long = "no-verify-mac")]
	no_verify_mac: bool,

	/// Do not use in memory sqlite database. Database is immediately created on disk (only considered with output type RAW).
	#[arg(long = "no-in-memory-db")]
	no_in_memory_db: bool,

	/// Backup password (30 digits, with or without spaces)
	#[arg(short = 'p', long = "password", value_name = "PASSWORD", group = "password")]
	password_string: Option<String>,

	/// File to read the backup password from
	#[arg(long = "password-file", value_name = "FILE", group = "password")]
	password_file: Option<std::path::PathBuf>,

	/// Read backup password from stdout from COMMAND
	#[arg(long = "password-command", value_name = "COMMAND", group = "password")]
	password_command: Option<String>,
}

/// Config struct
///
/// Stores all global variables
pub struct Config {
	/// Path to input file
	pub path_input: std::path::PathBuf,
	/// Path to output directory. If not given is automatically determined from input path.
	pub path_output: std::path::PathBuf,
	/// Password to open backup file
	pub password: Vec<u8>,
	/// Should HMAC be verified?
	pub verify_mac: bool,
	/// Log / verbosity level
	pub log_level: log::LevelFilter,
	/// Overwrite existing output files?
	pub force_overwrite: bool,
	/// Output type
	pub output_type: crate::output::SignalOutputType,
	/// Use in memory sqlite database
	pub output_raw_db_in_memory: bool,
}

impl Config {
	/// Create new config object
	pub fn new() -> Result<Self, anyhow::Error> {
		let args = Args::parse();

		// input file handling
		let input_file = args.input_file;

		// output path handling
		let output_path = if let Some(path) = args.output_path {
			path
		} else {
			std::path::PathBuf::from(
				input_file
					.file_stem()
					.context("Could not determine output path from input file")?
					.to_str()
					.context("Output path contains invalid characters")?,
			)
		};

		// password handling
		let mut password = {
			if let Some(pwd) = args.password_string {
				pwd
			} else if let Some(file_path) = args.password_file {
				let password_file = std::io::BufReader::new(
					std::fs::File::open(file_path).context("Unable to open password file")?,
				);
				password_file
					.lines()
					.next()
					.context("Password file is empty")?
					.context("Unable to read from password file")?
			} else if let Some(command) = args.password_command {
				let shell = std::env::var("SHELL").context("Could not determine current shell")?;
				let output = std::process::Command::new(shell)
					.arg("-c")
					.arg(command)
					.output()
					.context("Failed to execute password command")?;

				// check whether command returned an error code
				if output.status.success() {
					String::from_utf8(output.stdout)
						.context("Password command returned invalid characters")?
						.lines()
						.next()
						.context("Password command returned empty line")?
						.into()
				} else {
					return Err(anyhow!("Password command returned error code"));
				}
			} else {
				return Err(anyhow!("No password provided"));
			}
		};
		password.retain(|c| c >= '0' && c <= '9');
		let password = password.as_bytes().to_vec();
		if password.len() != 30 {
			return Err(anyhow!(
				"Wrong password length (30 numeric characters are expected)"
			));
		}

		// verbosity handling
		let log_level = if let Some(x) = args.log_level {
			match x.to_lowercase().as_str() {
				"debug" => log::LevelFilter::Debug,
				"info" => log::LevelFilter::Info,
				"warn" => log::LevelFilter::Warn,
				"error" => log::LevelFilter::Error,
				_ => return Err(anyhow!("Unknown log level given")),
			}
		} else {
			log::LevelFilter::Info
		};

		// determine output type
		let output_type = if let Some(x) = args.output_type {
			match x.to_lowercase().as_str() {
				"none" => crate::output::SignalOutputType::None,
				"raw" => crate::output::SignalOutputType::Raw,
				"csv" => crate::output::SignalOutputType::Csv,
				_ => return Err(anyhow!("Unknown output type given")),
			}
		} else {
			crate::output::SignalOutputType::Raw
		};

		Ok(Self {
			path_input: input_file,
			path_output: output_path,
			password,
			verify_mac: !args.no_verify_mac,
			log_level,
			force_overwrite: args.force_overwrite,
			output_type,
			output_raw_db_in_memory: !args.no_in_memory_db,
		})
	}
}
