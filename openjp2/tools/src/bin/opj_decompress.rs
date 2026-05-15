use openjp2::detect_format_from_file;
use openjp2_tools::jpeg2000_decode::decompress_image;
use openjp2_tools::params::{
  parse_decompress_options, DirContents, ImageFileFormat,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
  env_logger::init();
  let (params, img_folder) = match parse_decompress_options(std::env::args().collect())? {
    Some(opts) => opts,
    None => return Ok(()),
  };

  let start_time = std::time::Instant::now();
  let mut num_decompressed = 0;

  if let Some(dir) = img_folder.img_dir_path {
    let dir_contents = DirContents::new(&dir)?;

    for file in dir_contents.files {
      if detect_format_from_file(&file).is_ok() {
        println!("\nProcessing: {}", file.display());

        let mut file_params = params.clone();
        file_params.input_file = Some(file.clone());
        file_params.decode_format = ImageFileFormat::get_file_format(&file).ok();

        let stem = file.file_stem().ok_or("Invalid filename")?;
        let mut output = dir.join(stem);

        let ext = match img_folder.out_format.as_deref() {
          Some("PGX") => "pgx",
          Some("PGM") | Some("PPM") | Some("PNM") => "pnm",
          Some("BMP") => "bmp",
          Some("TIF") | Some("TIFF") => "tif",
          Some("RAW") => "raw",
          Some("RAWL") => "rawl",
          Some("TGA") => "tga",
          Some("PNG") => "png",
          _ => return Err("Invalid output format".into()),
        };
        output.set_extension(ext);
        file_params.output_file = Some(output.clone());

        decompress_image(file, output, &file_params)?;

        num_decompressed += 1;
      }
    }
  } else if let Some(input) = &params.input_file {
    let output = params.output_file.as_ref().ok_or("No output file")?;
    decompress_image(input, output, &params)?;
    num_decompressed += 1;
  }

  let elapsed = start_time.elapsed();
  if !params.quiet && num_decompressed > 0 {
    println!(
      "Decompressed {} files in {:.2} seconds",
      num_decompressed,
      elapsed.as_secs_f64()
    );
  }

  Ok(())
}
