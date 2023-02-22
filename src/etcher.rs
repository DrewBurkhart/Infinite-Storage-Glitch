use std::{fs, thread, vec};

use opencv::core::Mat;
use opencv::prelude::*;
use opencv::videoio::{VideoCapture, VideoWriter, CAP_ANY};

use crate::embedsource::EmbedSource;
use crate::settings::{Data, OutputMode, Settings};
use crate::timer::Timer;

//Get and write bytes from and to files. Start and end of app
//sounds cooler than og name (encode)
pub fn rip_bytes(path: &str) -> anyhow::Result<Vec<u8>> {
    let byte_data = fs::read(path)?;

    println!("Bytes ripped succesfully");
    println!("Byte length: {}", byte_data.len());
    Ok(byte_data)
}

pub fn rip_binary(byte_data: Vec<u8>) -> anyhow::Result<Vec<bool>> {
    let mut binary_data = Vec::with_capacity(byte_data.len() * 8);

    for byte in byte_data {
        let mut bits = [false; 8];
        for i in (0..8).rev() {
            bits[i] = (byte >> i) & 1 != 0;
        }
        binary_data.extend(bits);
    }

    println!("Binary ripped successfully");
    // println!("Binary length: {}", binary_data.len());
    Ok(binary_data)
}

pub fn rip_binary_u32(bytes: Vec<u32>) -> anyhow::Result<Vec<bool>> {
    let mut binary_data = Vec::with_capacity(bytes.len() * 32);

    for byte in bytes {
        for i in (0..32).rev() {
            binary_data.push((byte >> i) & 1 != 0);
        }
    }

    println!("Binary ripped successfully");
    // println!("Binary length: {}", binary_data.len());
    Ok(binary_data)
}

fn translate_u8(binary_data: Vec<bool>) -> anyhow::Result<Vec<u8>> {
    let mut buffer: Vec<bool> = Vec::with_capacity(8);
    let mut byte_data: Vec<u8> = Vec::with_capacity(binary_data.len() / 8);

    for bit in binary_data {
        buffer.push(bit);

        if buffer.len() == 8 {
            //idk how this works but it does
            let byte = buffer.iter().fold(0u8, |v, b| (v << 1) + (*b as u8));
            byte_data.push(byte);
            buffer = buffer[8..].to_vec();
        }
    }

    if !buffer.is_empty() {
        return Err(anyhow::anyhow!(
            "Invalid input: binary data has incomplete byte"
        ));
    }

    Ok(byte_data)
}

fn translate_u32(binary_data: Vec<bool>) -> anyhow::Result<Vec<u32>> {
    let mut buffer: Vec<bool> = Vec::with_capacity(32);
    let mut byte_data: Vec<u32> = Vec::with_capacity(binary_data.len() / 32);

    for bit in binary_data {
        buffer.push(bit);

        if buffer.len() == 32 {
            //idk how this works but it does
            let u32_byte = buffer.iter().fold(0u32, |v, b| (v << 1) + (*b as u32));
            byte_data.push(u32_byte);
            buffer = buffer[32..].to_vec();
        }
    }

    if !buffer.is_empty() {
        return Err(anyhow::anyhow!(
            "Invalid input: binary data has incomplete u32 value"
        ));
    }

    Ok(byte_data)
}

pub fn write_bytes(path: &str, data: Vec<u8>) -> anyhow::Result<()> {
    fs::write(path, data)?;
    println!("File written succesfully");
    Ok(())
}

//Returns average value of the pixel given size and location
fn get_pixel(frame: &EmbedSource, x: i32, y: i32) -> Option<Vec<u8>> {
    let mut r_sum = 0u32;
    let mut g_sum = 0u32;
    let mut b_sum = 0u32;

    let size = frame.size;
    for i in 0..size {
        for j in 0..size {
            let bgr = frame
                .image
                .at_2d::<opencv::core::Vec3b>(y + i, x + j)
                .unwrap();
            r_sum += bgr[2] as u32;
            g_sum += bgr[1] as u32;
            b_sum += bgr[0] as u32;
        }
    }

    let pixel_count = size * size;
    if pixel_count > 0 {
        let r_average = (r_sum / pixel_count as u32) as u8;
        let g_average = (g_sum / pixel_count as u32) as u8;
        let b_average = (b_sum / pixel_count as u32) as u8;
        let rgb_average = vec![r_average, g_average, b_average];
        // dbg!(&rgb_average);

        Some(rgb_average)
    } else {
        None
    }
}

//Draws the pixels, exists so you can draw bigger blocks
fn etch_pixel(frame: &mut EmbedSource, rgb: Vec<u8>, x: i32, y: i32) -> anyhow::Result<()> {
    for i in 0..frame.size {
        for j in 0..frame.size {
            // dbg!(x, y);
            let bgr = frame.image.at_2d_mut::<opencv::core::Vec3b>(y + i, x + j)?;
            //Opencv devs are reptilians who believe in bgr
            bgr[2] = rgb[0];
            bgr[1] = rgb[1];
            bgr[0] = rgb[2];
        }
    }

    Ok(())
}

fn etch_bw(
    source: &mut EmbedSource,
    data: &[bool],
    global_index: &mut usize,
) -> anyhow::Result<()> {
    let _timer = Timer::new("Etching frame");

    let width = source.actual_size.width;
    let height = source.actual_size.height;
    let size = source.size as usize;

    for y in (0..height).step_by(size) {
        for x in (0..width).step_by(size) {
            let local_index = *global_index;

            if let Some(&bit) = data.get(local_index) {
                let brightness = if bit { 255 } else { 0 };
                let rgb = vec![brightness, brightness, brightness];

                etch_pixel(source, rgb, x, y)?;
                *global_index += 1;
            } else {
                return Err(anyhow::anyhow!("Index beyond data"));
            }
        }
    }

    Ok(())
}

fn etch_color(
    source: &mut EmbedSource,
    data: &[u8],
    global_index: &mut usize,
) -> anyhow::Result<()> {
    let _timer = Timer::new("Etching frame");

    let width = source.actual_size.width;
    let height = source.actual_size.height;
    let size = source.size as usize;

    for y in (0..height).step_by(size) {
        for x in (0..width).step_by(size) {
            let local_index = *global_index;

            if let Some(slice) = data.get(local_index..local_index + 3) {
                let rgb = slice.to_vec();
                etch_pixel(source, rgb, x, y)?;
                *global_index += 3;
            } else {
                return Err(anyhow::anyhow!("Index beyond data"));
            }
        }
    }

    Ok(())
}

fn read_bw(
    source: &EmbedSource,
    current_frame: i32,
    final_frame: i32,
    final_bit: i32,
) -> anyhow::Result<Vec<bool>> {
    // let _timer = Timer::new("Dislodging frame");

    let width = source.actual_size.width;
    let height = source.actual_size.height;
    let size = source.size as usize;

    let mut binary_data: Vec<bool> = Vec::new();
    for y in (0..height).step_by(size) {
        for x in (0..width).step_by(size) {
            if let Some(rgb) = get_pixel(source, x, y) {
                let bit = rgb[0] >= 127;
                binary_data.push(bit);
            }
        }
    }

    //Cut off nasty bits at the end
    if current_frame == final_frame {
        if let Some(slice) = binary_data.get(..final_bit as usize) {
            return Ok(slice.to_vec());
        }
    }

    if let Some(slice) = binary_data.get(..) {
        return Ok(slice.to_vec());
    }

    // dbg!(binary_data.len());
    Ok(Vec::new())
}

fn read_color(
    source: &EmbedSource,
    current_frame: i32,
    final_frame: i32,
    final_byte: i32,
) -> anyhow::Result<Vec<u8>> {
    // let _timer = Timer::new("Dislodging frame");

    let width = source.actual_size.width;
    let height = source.actual_size.height;
    let size = source.size as usize;

    let mut byte_data: Vec<u8> = Vec::new();
    for y in (0..height).step_by(size) {
        for x in (0..width).step_by(size) {
            if let Some(rgb) = get_pixel(source, x, y) {
                byte_data.extend_from_slice(&rgb);
            }
        }
    }

    //Cut off nasty bits at the end
    if current_frame == final_frame {
        if let Some(slice) = byte_data.get(..final_byte as usize * 3) {
            return Ok(slice.to_vec());
        }
    }

    if let Some(slice) = byte_data.get(..) {
        return Ok(slice.to_vec());
    }

    Ok(Vec::new())
}

/*
Instructions:
Etched on first frame, always be wrtten in binary despite output mode
Output mode is the first byte
Size is constant 5
11111111 = Color (255), 00000000 = Binary(0),
Second byte will be the size of the pixels
FPS doesn't matter, but can add it anyways
Potentially add ending pointer so it doesn't make useless bytes
^^Currently implemented(?), unused
*/

fn etch_instructions(settings: &Settings, data: &Data) -> anyhow::Result<EmbedSource> {
    let instruction_size = 5;

    let mut u32_instructions: Vec<u32> = Vec::new();

    //calculating at what frame and pixel the file ends
    let frame_size = (settings.height * settings.width) as usize;

    //Adds the output mode to instructions
    //Instead of putting entire size of file, add at which frame and pixel file ends
    //Saves space on instruction frame
    match data.out_mode {
        OutputMode::Color => {
            u32_instructions.push(u32::MAX);

            let frame_data_size = frame_size / settings.size.pow(2) as usize;
            let final_byte = data.bytes.len() % frame_data_size;
            let mut final_frame = data.bytes.len() / frame_data_size;

            //In case of edge case where frame is right on the money
            if data.bytes.len() % frame_size != 0 {
                final_frame += 1;
            }

            dbg!(final_frame);
            u32_instructions.push(final_frame as u32);
            u32_instructions.push(final_byte as u32);
        }
        OutputMode::Binary => {
            u32_instructions.push(u32::MIN);

            let frame_data_size = frame_size / settings.size.pow(2) as usize;
            let final_byte = data.binary.len() % frame_data_size;
            let mut final_frame = data.binary.len() / frame_data_size;

            //In case of edge case where frame is right on the money
            if data.binary.len() % frame_size != 0 {
                final_frame += 1;
            }

            dbg!(final_frame);
            u32_instructions.push(final_frame as u32);
            u32_instructions.push(final_byte as u32);
        }
    };

    u32_instructions.push(settings.size as u32);
    u32_instructions.push(u32::MAX); //For some reason size not readable without this

    let instruction_data = rip_binary_u32(u32_instructions)?;

    let mut source = EmbedSource::new(instruction_size, settings.width, settings.height);
    let mut index = 0;
    match etch_bw(&mut source, &instruction_data, &mut index) {
        Ok(_) => {}
        Err(_) => {
            println!("Instructions written")
        }
    }

    // highgui::named_window("window", WINDOW_FULLSCREEN)?;
    // highgui::imshow("window", &source.image)?;
    // highgui::wait_key(10000000)?;

    // imwrite("src/out/test1.png", &source.image, &Vector::new())?;

    Ok(source)
}

fn read_instructions(
    source: &EmbedSource,
    threads: usize,
) -> anyhow::Result<(OutputMode, i32, i32, Settings)> {
    //UGLY
    let binary_data = read_bw(source, 0, 1, 0)?;
    let u32_data = translate_u32(binary_data)?;
    // dbg!(&u32_data);

    let out_mode = match u32_data[0] {
        u32::MAX => OutputMode::Color,
        _ => OutputMode::Binary,
    };

    let final_frame = u32_data[1] as i32;
    let final_byte = u32_data[2] as i32;
    let size = u32_data[3] as i32;

    let height = source.frame_size.height;
    let width = source.frame_size.width;

    let settings = Settings::new(size, threads, 1337, width, height);

    Ok((out_mode, final_frame, final_byte, settings))
}

pub fn etch(path: &str, data: Data, settings: Settings) -> anyhow::Result<()> {
    let _timer = Timer::new("Etching video");

    let mut spool = Vec::new();
    match data.out_mode {
        OutputMode::Color => {
            let length = data.bytes.len();

            //UGLY
            //Required so that data is continuous between each thread
            let frame_size = (settings.width * settings.height) as usize;
            let frame_data_size = frame_size / settings.size.pow(2) as usize * 3;
            let frame_length = length / frame_data_size;
            let chunk_frame_size = (frame_length / settings.threads) + 1;
            let chunk_data_size = chunk_frame_size * frame_data_size;

            //UGLY DUPING
            let chunks = data.bytes.chunks(chunk_data_size);
            for chunk in chunks {
                let chunk = chunk.to_vec();
                let thread = thread::spawn(move || {
                    let mut frames = Vec::new();
                    let mut index: usize = 0;

                    loop {
                        let mut source =
                            EmbedSource::new(settings.size, settings.width, settings.height);
                        match etch_color(&mut source, &chunk[index..], &mut index) {
                            Ok(_) => frames.push(source),
                            Err(_) => {
                                frames.push(source);
                                println!("Embedding thread complete!");
                                break;
                            }
                        }
                    }

                    frames
                });

                spool.push(thread);
            }
        }

        OutputMode::Binary => {
            let length = data.binary.len();

            //UGLY
            //Required so that data is continuous between each thread
            let frame_size = (settings.width * settings.height) as usize;
            let frame_data_size = frame_size / settings.size.pow(2) as usize;
            let frame_length = length / frame_data_size;
            let chunk_frame_size = (frame_length / settings.threads) + 1;
            let chunk_data_size = chunk_frame_size * frame_data_size;

            //UGLY DUPING
            let chunks = data.binary.chunks(chunk_data_size);
            for chunk in chunks {
                let chunk = chunk.to_vec();
                let thread = thread::spawn(move || {
                    let mut frames = Vec::new();
                    let mut index: usize = 0;

                    loop {
                        let mut source =
                            EmbedSource::new(settings.size, settings.width, settings.height);
                        match etch_bw(&mut source, &chunk[index..], &mut index) {
                            Ok(_) => frames.push(source),
                            Err(_) => {
                                frames.push(source);
                                println!("Embedding thread complete!");
                                break;
                            }
                        }
                    }

                    frames
                });

                spool.push(thread);
            }
        }
    }

    let mut complete_frames = Vec::new();

    let instructional_frame = etch_instructions(&settings, &data)?;
    complete_frames.push(instructional_frame);

    for thread in spool {
        let frame_chunk = thread.join().unwrap();
        complete_frames.extend(frame_chunk);
    }

    //Mess around with lossless codecs, png seems fine
    //Fourcc is a code for video codecs, trying to use a lossless one
    let fourcc = VideoWriter::fourcc('p', 'n', 'g', ' ')?;
    // let fourcc = VideoWriter::fourcc('j', 'p', 'e', 'g')?;
    // let fourcc = VideoWriter::fourcc('a', 'v', 'c', '1')?;

    //Check if frame_size is flipped
    let frame_size = complete_frames[1].frame_size;
    let mut video = VideoWriter::new(path, fourcc, settings.fps, frame_size, true)?;

    //Putting them in vector might be slower
    for frame in complete_frames {
        let image = frame.image;
        video.write(&image)?;
    }
    video.release()?;

    println!("Video embedded successfully at {}", path);

    Ok(())
}

pub fn read(path: &str, threads: usize) -> anyhow::Result<Vec<u8>> {
    let _timer = Timer::new("Dislodging frame");
    let instruction_size = 5;

    let mut video = VideoCapture::from_file(path, CAP_ANY).expect("Could not open video path");
    let mut frame = Mat::default();

    video.read(&mut frame)?;
    let instruction_source = EmbedSource::from(frame, instruction_size);
    let (out_mode, final_frame, final_byte, settings) =
        read_instructions(&instruction_source, threads)?;

    let mut byte_data = Vec::new();
    let mut current_frame = 1;
    loop {
        let mut frame = Mat::default();

        video.read(&mut frame)?;
        if frame.cols() == 0 {
            break;
        }

        if current_frame % 20 == 0 {
            println!("On frame: {}", current_frame);
        }

        let source = EmbedSource::from(frame, settings.size);

        let frame_data = match out_mode {
            OutputMode::Color => read_color(&source, current_frame, 99999999, final_byte)?,
            OutputMode::Binary => {
                let binary_data = read_bw(&source, current_frame, final_frame, final_byte)?;
                translate_u8(binary_data)?
            }
        };

        byte_data.extend_from_slice(&frame_data);
        current_frame += 1;
    }

    println!("Video read successfully");
    Ok(byte_data)
}

//Uses literally all the RAM
// pub fn read(path: &str, threads: usize) -> anyhow::Result<Vec<u8>> {
//     let _timer = Timer::new("Dislodging frame");
//     let instruction_size = 5;

//     let mut video = VideoCapture::from_file(&path, CAP_ANY)
//             .expect("Could not open video path");
//     let mut frame = Mat::default();

//     //Could probably avoid cloning
//     video.read(&mut frame)?;
//     let instruction_source = EmbedSource::from(frame.clone(), instruction_size);
//     let (out_mode, final_frame, final_byte, settings) = read_instructions(&instruction_source, threads)?;

//     let mut frames: Vec<Mat> = Vec::new();
//     loop {
//         // let _timer = Timer::new("Reading frame  (clone included)");
//         video.read(&mut frame)?;

//         //If it reads an empty image, the video stopped
//         if frame.cols() == 0 {
//             break;
//         }

//         frames.push(frame.clone());
//     }

//     //Required so that data is continuous between each thread
//     let chunk_size = (frames.len() / settings.threads) + 1;

//     let mut spool = Vec::new();
//     let chunks = frames.chunks(chunk_size);
//     //Can get rid of final_frame because of this
//     for chunk in chunks {
//         let chunk_copy = chunk.to_vec();
//         //Checks if this is final thread
//         let final_frame = if spool.len() == settings.threads - 1 {
//             chunk_copy.len() as i32
//         } else {
//             -1
//         };

//         let thread = thread::spawn(move || {
//             let mut byte_data = Vec::new();
//             let mut current_frame = 1;

//             for frame in chunk_copy {
//                 let source = EmbedSource::from(frame, settings.size);

//                 let frame_data = match out_mode {
//                     OutputMode::Color => {
//                         read_color(&source, current_frame, final_frame, final_byte).unwrap()
//                     },
//                     OutputMode::Binary => {
//                         let binary_data = read_bw(&source, current_frame, final_frame, final_byte).unwrap();
//                         translate_u8(binary_data).unwrap()
//                     }
//                 };
//                 current_frame += 1;

//                 byte_data.extend(frame_data);
//             }

//             println!("Dislodging thread complete!");
//             return byte_data;
//         });

//         spool.push(thread);
//     }

//     let mut complete_data = Vec::new();
//     for thread in spool {
//         let byte_chunk = thread.join().unwrap();
//         complete_data.extend(byte_chunk);
//     }

//     println!("Video read succesfully");
//     return Ok(complete_data);
// }
