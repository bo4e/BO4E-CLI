pub mod marker;
pub mod paths;
pub mod shells;
pub mod install;
pub mod uninstall;
pub mod show;

#[cfg(feature = "dynamic-completion")]
pub mod completers;
