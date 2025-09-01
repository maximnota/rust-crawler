use fantoccini::{Client, Locator};
use std::collections::{HashSet, VecDeque};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use url::Url;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut c = Client::new("http://localhost:4444").await?;

    // Input start URLs
    println!("Enter start URLs separated by commas:");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let start_urls: Vec<String> = input
        .trim()
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    // Input keywords
    println!("Enter keywords separated by commas:");
    let mut input_keywords = String::new();
    std::io::stdin().read_line(&mut input_keywords)?;
    let keywords: Vec<String> = input_keywords
        .trim()
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .collect();

    let mut queue: VecDeque<String> = VecDeque::from(start_urls.clone());
    let mut visited: HashSet<String> = HashSet::new();

    // Open file async
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open("urls.txt")
        .await?;

    while let Some(url) = queue.pop_front() {
        if visited.contains(&url) {
            continue;
        }
        visited.insert(url.clone());
        println!("Crawling: {}", url);

        if let Err(e) = c.goto(&url).await {
            eprintln!("Failed to navigate to {}: {}", url, e);
            continue;
        }

        // Handle alerts
        if let Ok(alert_text) = c.get_alert_text().await {
            println!("Alert appeared: {}", alert_text);
            // Try to send text, but only catch the error if not a prompt
            if let Err(e) = c.send_alert_text("RustUser").await {
                println!("Not a prompt, cannot send text: {}", e);
            }
            c.accept_alert().await?;
        }

        // Get page text
        let page_text = if let Ok(elem) = c.find(Locator::Css("body")).await {
            elem.text().await.unwrap_or_default().to_lowercase()
        } else {
            String::new()
        };

        // Check for keywords
        let found_keywords: Vec<String> = keywords
            .iter()
            .filter(|kw| page_text.contains(*kw))
            .cloned()
            .collect();

        if !found_keywords.is_empty() {
            let line = format!("{} => {:?}\n", url, found_keywords);
            file.write_all(line.as_bytes()).await?;
            file.flush().await?;
            println!("Saved {} with keywords {:?}", url, found_keywords);
        }

        // Function to normalize links
        let normalize_link = |link: &str| -> Option<String> {
            if let Ok(abs) = Url::parse(link) {
                Some(abs.to_string())
            } else if let Ok(base) = Url::parse(&url) {
                base.join(link).ok().map(|u| u.to_string())
            } else {
                None
            }
        };

        // Crawl <a> links
        if let Ok(elements) = c.find_all(Locator::Css("a")).await {
            for elem in elements {
                if let Ok(Some(href)) = elem.attr("href").await {
                    if let Some(normalized) = normalize_link(&href) {
                        if !visited.contains(&normalized) {
                            queue.push_back(normalized);
                        }
                    }
                }
            }
        }

        // Crawl <button> links
        if let Ok(elements) = c.find_all(Locator::Css("button")).await {
            for elem in elements {
                if let Ok(Some(data_url)) = elem.attr("data-url").await {
                    if let Some(normalized) = normalize_link(&data_url) {
                        if !visited.contains(&normalized) {
                            queue.push_back(normalized);
                        }
                    }
                }
                if let Ok(Some(onclick)) = elem.attr("onclick").await {
                    let mut rest = &onclick[..];
                    while let Some(start) = rest.find("http") {
                        let end_offset = rest[start..]
                            .find(|c| c == '\'' || c == '"' || c == ')')
                            .unwrap_or(rest[start..].len());
                        let found_url = &rest[start..start + end_offset];
                        if let Some(normalized) = normalize_link(found_url) {
                            if !visited.contains(&normalized) {
                                queue.push_back(normalized);
                            }
                        }
                        rest = &rest[start + end_offset..];
                    }
                }
            }
        }
    }

    c.close().await?;
    println!("Crawled {} unique URLs.", visited.len());
    println!("URLs with keywords saved to urls.txt");
    Ok(())
}
