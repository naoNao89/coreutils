// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#[cfg(all(feature = "selinux", any(target_os = "linux", target_os = "android")))]
uucore::bin!(uu_runcon);

#[cfg(not(all(feature = "selinux", any(target_os = "linux", target_os = "android"))))]
fn main() {
    eprintln!("runcon: SELinux is not supported on this platform");
    std::process::exit(1);
}
