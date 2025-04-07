use std::fs::File;
use std::io::{Read, Write};
use std::net::TcpStream;
use sha2::{Sha256, Digest};

fn main() -> std::io::Result<()> {
    let host = "127.0.0.1:8080";
    let output_file = "downloaded_data.bin";
    
    // First find out the expected total size
    let total_size = get_total_size(host)?;
    println!("Detected total size: {} bytes", total_size);
    
    let mut all_data = Vec::with_capacity(total_size);
    let mut file = File::create(output_file)?;
    let mut position = 0;
    
    // Download until we've reached the total size
    while position < total_size {
        let chunk = download_chunk(host, position)?;
        
        if chunk.is_empty() {
            println!("Warning: Received empty chunk, retrying");
            continue;
        }
        
        file.write_all(&chunk)?;
        all_data.extend_from_slice(&chunk);
        position += chunk.len();
        
        println!("Downloaded: {}/{} bytes", position, total_size);
    }
    
    // Verify we got the expected amount of data
    if all_data.len() != total_size {
        println!("Warning: Downloaded size ({}) doesn't match expected size ({})",
                all_data.len(), total_size);
    }
    
    // Calculate SHA-256 hash
    let mut hasher = Sha256::new();
    hasher.update(&all_data);
    let hash = format!("{:x}", hasher.finalize());
    
    println!("Download complete. SHA-256 hash: {}", hash);
    println!("Verify this hash matches what the server displayed");
    
    Ok(())
}

// Get the total size of the content
fn get_total_size(host: &str) -> std::io::Result<usize> {
    // Make a full request first to get the total size
    let request = "GET / HTTP/1.1\r\nHost: 127.0.0.1:8080\r\nConnection: close\r\n\r\n";
    
    let mut conn = TcpStream::connect(host)?;
    conn.write_all(request.as_bytes())?;
    
    // We don't need to read all the data, just the headers
    let mut response = Vec::new();
    let mut buffer = [0; 1024];
    
    // Read just enough to get the headers
    loop {
        match conn.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                response.extend_from_slice(&buffer[0..n]);
                // If we have the headers, we can stop
                if response.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(e) => return Err(e),
        }
    }
    
    // Parse the headers to find Content-Length
    let headers = String::from_utf8_lossy(&response);
    let content_length = headers.lines()
        .find(|line| line.to_lowercase().starts_with("content-length:"))
        .and_then(|line| line.split(':').nth(1))
        .and_then(|len| len.trim().parse::<usize>().ok())
        .ok_or(std::io::Error::new(std::io::ErrorKind::Other, "No Content-Length header"))?;
    
    Ok(content_length)
}

// Download a chunk of data starting at the specified position
fn download_chunk(host: &str, start_position: usize) -> std::io::Result<Vec<u8>> {
    let chunk_size = 64 * 1024; // 64KB chunks
    let end_position = start_position + chunk_size - 1;
    
    let range = format!("bytes={}-{}", start_position, end_position);
    let request = format!(
        "GET / HTTP/1.1\r\nHost: 127.0.0.1:8080\r\nRange: {}\r\nConnection: close\r\n\r\n", 
        range
    );
    
    let mut conn = TcpStream::connect(host)?;
    conn.write_all(request.as_bytes())?;
    
    let mut response = Vec::new();
    let mut buffer = [0; 4096];
    
    // Read the entire response
    loop {
        match conn.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => response.extend_from_slice(&buffer[0..n]),
            Err(e) => return Err(e),
        }
    }
    
    // Check if we got a valid response
    if response.is_empty() {
        return Ok(Vec::new());
    }
    
    // Extract just the body
    Ok(extract_body(&response))
}

// Extract the HTTP body from a complete HTTP response
fn extract_body(response: &[u8]) -> Vec<u8> {
    // Look for the double CRLF that separates headers from body
    let mut i = 0;
    while i + 3 < response.len() {
        if &response[i..i+4] == b"\r\n\r\n" {
            return response[i+4..].to_vec();
        }
        i += 1;
    }
    
    // If we can't find the separator, return an empty vector
    // This is safer than returning potentially incorrect data
    Vec::new()
}