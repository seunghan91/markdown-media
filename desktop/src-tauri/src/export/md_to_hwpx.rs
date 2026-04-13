use std::io;
use std::path::Path;

pub fn export(_markdown: &str, _template: &str, _output: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "HWPX export is planned for Phase 2 and is not implemented yet.",
    ))
}
