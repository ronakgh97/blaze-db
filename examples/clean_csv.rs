use anyhow::Result;
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
struct CsvDataPoint {
    id: String,
    title: String,
    #[serde(rename = "imgUrl")]
    img_url: String,
    #[serde(rename = "productURL")]
    product_url: String,
    stars: String,
    reviews: String,
    price: String,
    #[serde(rename = "listPrice")]
    list_price: String,
    category_id: String,
    #[serde(rename = "isBestSeller")]
    is_best_seller: String,
    #[serde(rename = "boughtInLastMonth")]
    bought_in_last_month: String,
}

fn main() -> Result<()> {
    let input_path = "datasets/amazon_products.csv";
    let output_path = "datasets/amazon_products_clean.csv";

    let mut rdr = csv::Reader::from_path(input_path)?;
    let mut seen_titles = HashSet::new();
    let mut unique_records = Vec::new();

    let mut total_count = 0;
    let mut duplicate_count = 0;

    for result in rdr.deserialize() {
        total_count += 1;
        let record: CsvDataPoint = result?;

        if seen_titles.insert(record.title.clone()) {
            unique_records.push(record);
        } else {
            duplicate_count += 1;
            if duplicate_count <= 20 {
                println!("Duplicate #{}: {}", duplicate_count, record.title);
            }
        }
    }

    println!("Total records: {}", total_count);
    println!("Duplicate records: {}", duplicate_count);
    println!("Unique records: {}", unique_records.len());

    // Check duplicates or exit early
    if duplicate_count == 0 {
        println!("No duplicates found, Exiting");
        return Ok(());
    }

    // Write the cleaned data to a new CSV file
    let mut wtr = csv::Writer::from_path(output_path)?;

    // Write header
    wtr.write_record([
        "id",
        "title",
        "imgUrl",
        "productURL",
        "stars",
        "reviews",
        "price",
        "listPrice",
        "category_id",
        "isBestSeller",
        "boughtInLastMonth",
    ])?;

    // Write unique records
    for record in unique_records {
        wtr.write_record([
            &record.id,
            &record.title,
            &record.img_url,
            &record.product_url,
            &record.stars,
            &record.reviews,
            &record.price,
            &record.list_price,
            &record.category_id,
            &record.is_best_seller,
            &record.bought_in_last_month,
        ])?;
    }

    wtr.flush()?;

    println!("\nCleaned CSV written to: {}", output_path);

    Ok(())
}
