mod bitstream_util;
mod h264_parser;

use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = &args[1];
    let in_filename = &args[2];
    let out_filename = &args[3];

    if mode == "-e" {
        let human_readable = fs::read_to_string(in_filename).expect("Cannot read file");
        let bytes = h264_parser::serialize_h264(human_readable);
        fs::write(out_filename, bytes).expect("Cannot write file");
    } else if mode == "-d" {
        let bytes = fs::read(in_filename).expect("Cannot read file");
        let nalus = h264_parser::parse_h264(&bytes);
        let mut human_readable = "".to_string();
        for nalu in &nalus {
            human_readable = format!("{}{}", human_readable, nalu.to_string());
        }
        fs::write(out_filename, human_readable).expect("Cannot write file");
    } else {
        panic!("Invalid flag {}", mode);
    }
}
