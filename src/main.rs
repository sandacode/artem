use std::{fs::File, io::Write, path::Path};

use log::{debug, info, trace, warn, LevelFilter};

use crate::conversion_options::ConversionOptionBuilder;

//import cli
mod cli;
//import utilities
mod util;

//condense all arguments into a single struct
mod conversion_options;

//import functions for conversion
mod image_conversion;

fn main() {
    //get args from cli
    let matches = cli::build_cli().get_matches();

    //get log level from args
    let log_level = match matches.value_of("verbosity") {
        Some("trace") => LevelFilter::Trace,
        Some("debug") => LevelFilter::Debug,
        Some("info") => LevelFilter::Info,
        Some("warn") => LevelFilter::Warn,
        Some("error") => LevelFilter::Error,
        _ => LevelFilter::Error,
    };

    //enable logging
    env_logger::builder()
        .format_target(false)
        .format_timestamp(None)
        .filter_level(log_level)
        .init();
    trace!("Started logger with trace");

    let mut options_builder = ConversionOptionBuilder::new();

    //this should be save to unwrap since the input has to be non-null
    let img_path = matches.value_of("INPUT").unwrap();
    //check if file exist
    if !Path::new(img_path).exists() {
        util::fatal_error(format!("File {img_path} does not exist").as_str(), Some(66));
    } else if !Path::new(img_path).is_file() {
        util::fatal_error(format!("{img_path} is not a file").as_str(), Some(66));
    }

    //try to open img
    let img = match image::open(img_path) {
        Ok(img) => img,
        Err(err) => util::fatal_error(err.to_string().as_str(), Some(66)),
    };

    //density char map
    let density = if matches.is_present("density") {
        match matches.value_of("density").unwrap() {
            "short" | "s" | "0" => r#"Ñ@#W$9876543210?!abc;:+=-,._ "#,
            "flat" | "f" | "1" => r#"MWNXK0Okxdolc:;,'...   "#,
            "long" | "l" | "2" => {
                r#"$@B%8&WM#*oahkbdpqwmZO0QLCJUYXzcvunxrjft/\|()1{}[]?-_+~<>i!lI;:,"^`'. "#
            }
            _ => {
                info!("Using user provided characters");
                matches.value_of("density").unwrap()
            }
        }
    } else {
        //density map from jp2a
        info!("Using default characters");
        r#"MWNXK0Okxdolc:;,'...   "#
    };
    debug!("Characters used: \"{density}\"");
    options_builder = options_builder.density(density);

    //set the default resizing dimension to width
    options_builder = options_builder.dimension(util::ResizingDimension::Width);

    //get target size from args
    //only one arg should be present
    let target_size = if matches.is_present("height") {
        //use max terminal height
        trace!("Using terminal height as target size");
        //change dimension to height
        options_builder = options_builder.dimension(util::ResizingDimension::Height);
        terminal_size::terminal_size().unwrap().1 .0 as u32
    } else if matches.is_present("width") {
        //use max terminal width
        trace!("Using terminal width as target size");
        terminal_size::terminal_size().unwrap().0 .0 as u32
    } else {
        //use given input size
        trace!("Using user input size as target size");
        match matches
            .value_of("size")
            .unwrap() //this should always be at least "80", so it should be safe to unwrap
            .parse::<u32>()
        {
            Ok(v) => v.clamp(
                20,  //min should be 20 to ensure a somewhat visible picture
                230, //img above 230 might not be displayed properly
            ),
            Err(_) => util::fatal_error("Could not work with size input value", Some(65)),
        }
    };
    debug!("Target Size: {target_size}");
    options_builder = options_builder.target_size(target_size);

    //best ratio between height and width is 0.43
    let scale = match matches
        .value_of("scale")
        .unwrap() //this should always be at least "0.43", so it should be safe to unwrap
        .parse::<f64>()
    {
        Ok(v) => v.clamp(
            0f64, //a negative scale is not allowed
            1f64, //even a scale above 0.43 is not looking good
        ),
        Err(_) => util::fatal_error("Could not work with ratio input value", Some(65)),
    };
    debug!("Scale: {scale}");
    options_builder = options_builder.scale(scale);

    //number rof threads used to convert the image
    let thread_count: u32 = match matches
        .value_of("threads")
        .unwrap() //this should always be at least "4", so it should be safe to unwrap
        .parse::<u32>()
    {
        Ok(v) => v,
        Err(_) => util::fatal_error("Could not work with thread input value", Some(65)),
    };
    options_builder = options_builder.thread_count(thread_count);

    if !matches.is_present("no-color") && matches.is_present("output-file") {
        warn!("Output-file flag is present, ignoring colors")
    }

    let invert = matches.is_present("invert-density");
    debug!("Invert is set to: {invert}");
    options_builder = options_builder.invert(invert);

    let on_background_color = matches.is_present("background-color");
    debug!("BackgroundColor is set to: {on_background_color}");
    options_builder = options_builder.on_background(on_background_color);

    //check if no colors should be used or the if a output file will be used
    //since text documents don`t support ansi ascii colors
    let color = if matches.is_present("no-color") || matches.is_present("output-file") {
        //print the "normal" non-colored conversion
        info!("Using non-colored ascii");
        false
    } else {
        //print colored terminal conversion, this should already respect truecolor support/use ansi colors if not supported
        info!("Using colored ascii");
        let truecolor = util::supports_truecolor();
        if !truecolor {
            if on_background_color {
                warn!("Background flag will be ignored, since truecolor is not supported.")
            }
            warn!("Truecolor is not supported. Using ansi color")
        } else {
            info!("Using truecolor ascii")
        }
        true
    };
    options_builder = options_builder.color(color);

    //get flag for border around image
    let border = matches.is_present("border");
    options_builder = options_builder.border(border);
    info!("Using border: {border}");

    //get flags for flipping along x axis
    let transform_x = matches.is_present("flipX");
    options_builder = options_builder.transform_x(transform_x);
    debug!("Flipping X-Axis: {transform_x}");

    //get flags for flipping along y axis
    let transform_y = matches.is_present("flipY");
    options_builder = options_builder.transform_y(transform_y);
    debug!("Flipping Y-Axis: {transform_y}");

    // //get output file extension, will be empty if non is specified
    // let file_path = PathBuf::from(matches.value_of("output-file").unwrap_or_default());
    // let file_extension = file_path.extension().unwrap_or_default().to_str();

    //convert the img to ascii string
    info!("Converting the img: {img_path}");
    // let output = match file_extension {
    //     Some("html") => "".to_string(),
    //     _ => image_conversion::convert_img(img, options_builder.build()),
    // };
    let output = image_conversion::convert_img(img, options_builder.build());

    //create and write to output file
    if matches.is_present("output-file") && matches.value_of("output-file").is_some() {
        info!("Writing output to output file");
        let mut file = match File::create(matches.value_of("output-file").unwrap()) {
            Ok(f) => f,
            Err(_) => util::fatal_error("Could not create file", Some(73)),
        };
        trace!("Created output file");

        match file.write(output.as_bytes()) {
            Ok(result) => {
                info!("Written ascii chars to output file");
                println!(
                    "Written {result} bytes to {}",
                    matches.value_of("output-file").unwrap()
                )
            }
            Err(_) => util::fatal_error("Could not write to file", Some(74)),
        };
    } else {
        //print the img to the terminal
        info!("Printing output");
        println!("{output}");
    }
}
