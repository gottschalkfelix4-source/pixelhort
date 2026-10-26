//! Zero-shot object/scene tagging using CLIP embeddings.

/// Built-in vocabulary. Prompts are "a photo of a {label}".
pub const LABELS: &[&str] = &[
    // people
    "person", "baby", "child", "man", "woman", "group of people", "crowd", "selfie", "portrait", "wedding",
    "birthday party", "concert",
    // animals
    "dog", "cat", "bird", "horse", "cow", "sheep", "pig", "chicken", "duck", "fish", "rabbit", "hamster",
    "squirrel", "deer", "bear", "elephant", "giraffe", "zebra", "lion", "tiger", "monkey", "butterfly",
    "insect", "spider", "snake", "turtle", "frog",
    // vehicles
    "car", "truck", "bus", "motorcycle", "bicycle", "train", "airplane", "boat", "ship", "tractor",
    // nature & landscape
    "beach", "ocean", "lake", "river", "waterfall", "mountain", "forest", "tree", "flower", "garden", "park",
    "field", "desert", "snow", "sunset", "sunrise", "sky", "clouds", "rainbow", "night sky", "island",
    "cave", "volcano", "meadow",
    // places & buildings
    "city", "street", "building", "skyscraper", "house", "church", "castle", "bridge", "tower", "museum",
    "restaurant", "kitchen", "living room", "bedroom", "bathroom", "office", "classroom", "shop",
    "supermarket", "stadium", "swimming pool", "playground", "airport", "train station", "road", "harbor",
    // food & drink
    "food", "pizza", "burger", "pasta", "salad", "sushi", "cake", "bread", "fruit", "vegetables",
    "ice cream", "dessert", "coffee", "wine", "beer", "cocktail", "breakfast", "barbecue",
    // objects
    "phone", "laptop", "computer", "television", "camera", "book", "newspaper", "document", "screenshot",
    "receipt", "sign", "map", "painting", "sculpture", "toy", "teddy bear", "ball", "guitar", "piano",
    "clock", "chair", "sofa", "table", "bed", "lamp", "window", "door", "umbrella", "backpack", "bottle",
    "cup", "glasses", "hat", "shoes", "dress", "watch", "christmas tree", "fireworks", "candle", "balloon",
    // activities & sports
    "soccer", "tennis", "basketball", "skiing", "snowboarding", "surfing", "swimming", "hiking", "camping",
    "cycling", "running", "fishing", "dancing",
];

/// Returns labels with softmax(100 * cosine) >= `min_score`, best first, at most `top_k`.
pub fn classify(image_emb: &[f32], label_embs: &[Vec<f32>], min_score: f32, top_k: usize) -> Vec<(&'static str, f32)> {
    let logits: Vec<f32> = label_embs
        .iter()
        .map(|t| 100.0 * t.iter().zip(image_emb).map(|(a, b)| a * b).sum::<f32>())
        .collect();
    let max = logits.iter().copied().fold(f32::MIN, f32::max);
    let exps: Vec<f32> = logits.iter().map(|l| (l - max).exp()).collect();
    let sum: f32 = exps.iter().sum();
    let mut scored: Vec<(&'static str, f32)> =
        LABELS.iter().zip(exps).map(|(l, e)| (*l, e / sum)).filter(|(_, s)| *s >= min_score).collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.truncate(top_k);
    scored
}
