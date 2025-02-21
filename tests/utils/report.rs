use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

pub struct Report {
    pub report_path: PathBuf,
}

impl Report {
    pub fn write_header(&self, fields: &[&str]) {
        let mut file = std::fs::File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&self.report_path)
            .unwrap();
        file.write_all(format!("{}\n", fields.join(";")).as_bytes())
            .unwrap();
    }

    pub fn write_duration(&self, duration: Duration) {
        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.report_path)
            .unwrap();
        file.write_all(format!("{};", duration.as_millis()).as_bytes())
            .unwrap();
    }

    pub fn end_line(&self) {
        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.report_path)
            .unwrap();
        file.write_all("\n".as_bytes()).unwrap();
    }
}
