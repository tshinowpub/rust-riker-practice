use std::env;
use std::process::exit;

mod executor;
mod commands;
pub mod command;

use crate::executor::Executor;

fn main() {
    let execute_path: String;
    match env::current_exe() {
        Ok(exe_path) =>
            execute_path = exe_path.display().to_string(),
        Err(e) => {
            println!("Failed to get current execute path: {e}.");

            exit(1);
        },
    };

    let arguments = extract_user_arguments(env::args().collect(), &execute_path);
    let options = extract_options(env::args().collect(), &execute_path);

    match arguments.clone().first() {
        Some(command)    => {
            Executor::execute(command, &arguments, &options);
        }
        None => {
            println!("Use --help.");

            exit(1);
        }
    }

    exit(0);
}

fn extract_user_arguments(arguments: Vec<String>, execute_path: &String) -> Vec<String> {
    let user_arguments: Vec<String> = arguments
        .iter()
        .filter_map(|s| {
            return match s {
                s if (s != execute_path && !is_option(s)) => s.parse::<String>().ok(),
                _ => None
            }
        })
        .collect();

    user_arguments
}

fn extract_options(arguments: Vec<String>, execute_path: &String) -> Vec<String> {
    return arguments
        .iter()
        .filter_map(|s| {
            return match s {
                s if (s != execute_path && is_option(s)) => s.parse::<String>().ok(),
                _ => None
            }
        })
        .collect();
}

fn is_option(pattern: &String) -> bool {
    return match pattern.find("-") {
        Some(found) if found == 0 => true,
        _ => false
    }
}
