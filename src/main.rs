use find_program_by_name::find_program_by_name;
use std::env;
use std::io;
use std::process::{exit, Command, ExitStatus};

#[cfg(unix)]
fn signal_error(status: ExitStatus) -> io::Error {
    use std::ffi::CStr;
    use std::os::unix::process::ExitStatusExt;

    let signal_num = status.signal().unwrap() as libc::c_int;
    let mut message = String::new();
    unsafe {
        let signal_description = CStr::from_ptr(libc::strsignal(signal_num));
        message.push_str(&signal_description.to_string_lossy());

        #[cfg(feature = "unix_process_wait_more")]
        if status.core_dumped() {
            message.push_str(" (core dumped)");
        }
    }
    io::Error::new(io::ErrorKind::Other, message)
}

#[cfg(not(unix))]
fn signal_error(_status: ExitStatus) -> io::Error {
    unreachable!("Signals? On my Windows?")
}

fn main() {
    let mut args = env::args();
    args.next();
    let (expect_crash, program) = match args.next() {
        Some(arg) if arg == "--crash" => {
            // Crash is expected, so disable crash report and symbolization to reduce
            // output and avoid potentially slow symbolization.
            env::set_var("LLVM_DISABLE_CRASH_REPORT", "1");
            env::set_var("LLVM_DISABLE_SYMBOLIZATION", "1");
            (true, None)
        }
        Some(program) => (false, Some(program)),
        None => exit(1),
    };

    let program = match program.or_else(|| args.next()) {
        Some(program_name) => match find_program_by_name(&program_name) {
            Ok(program) => program,
            Err(error) => {
                // TODO: use color
                eprintln!(
                    "error: unable to find '{}' in PATH: {}",
                    program_name, error
                );
                exit(1)
            }
        },
        None => exit(1),
    };

    let print_error_and_exit = |error| -> ! {
        // TODO: use color
        eprintln!("error: {}", error);
        if expect_crash {
            exit(0);
        }
        exit(1);
    };

    match Command::new(program).args(args).status() {
        Ok(exit_status) => match exit_status.code() {
            Some(code) => {
                #[cfg(windows)]
                if expect_crash && code == 3 {
                    // Handle abort() in msvcrt -- It has exit code as 3.  abort(), aka
                    // unreachable, should be recognized as a crash.  However, some binaries use
                    // exit code 3 on non-crash failure paths, so only do this if we expect a
                    // crash.
                    print_error_and_exit(io::Error::from(io::ErrorKind::Other))
                }

                if expect_crash || code == 0 {
                    exit(1)
                } else {
                    exit(0)
                }
            }
            None => print_error_and_exit(signal_error(exit_status)),
        },
        Err(error) => print_error_and_exit(error),
    };
}
