# pixelhort-ml

The Pixelhort machine-learning service. It computes CLIP embeddings for smart search, detects and embeds faces, runs OCR, and tags objects/scenes. The main server calls it over HTTP (`PH_ML_URL`), so it can run on a separate machine with a GPU.

It is written in Rust (axum + ONNX Runtime via `ort`). Models are downloaded from Hugging Face on first use, cached on disk, loaded lazily and unloaded again when idle.

## API

The binding contract is the "ML service API" section of [`docs/API.md`](../../docs/API.md).

| Method | Path | Auth | Description |
|---|---|---|---|
| GET | `/ping` | no | `pong` |
| GET | `/health` | no | `{"status":"ok","models":{"clip":true,"faces":true,"ocr":true}, ...}`. The extra `loaded` object shows which models are currently in memory. |
| GET | `/metrics` | no | Prometheus metrics |
| POST | `/predict` | yes | multipart: `image` (JPEG/PNG/WebP), `tasks` (comma list of `clip,faces,ocr,objects`; default all), `faceMinScore` (default 0.7) |
| POST | `/clip/text` | yes | `{"text":"a dog"}` → `{"embedding":[512 floats]}` |

"Auth" means that when `PH_ML_API_KEY` is set, the request must carry the header `x-ml-key: <key>`. Without the key the service answers `401`.

Errors use the server's format: `{"statusCode":400,"error":"Bad Request","message":"..."}`.

Notes on `/predict`:

* EXIF orientation is applied before inference. `imageWidth`/`imageHeight` and all boxes refer to the upright image, which is the same as the raw pixels for images without an orientation tag (for example server-generated previews).
* Face boxes and OCR quads are integer pixel coordinates. OCR `box` holds the corners top-left, top-right, bottom-right, bottom-left.
* `objects` needs the CLIP image embedding. If only `objects` is requested, CLIP still runs, but `clip` stays `null` in the response.
* Requested tasks run concurrently on the blocking thread pool.

```sh
curl -s localhost:3003/ping
curl -s -X POST localhost:3003/clip/text -H 'content-type: application/json' -d '{"text":"a dog on the beach"}'
curl -s -X POST localhost:3003/predict -F image=@photo.jpg -F tasks=clip,faces,ocr,objects -F faceMinScore=0.7
```

## Configuration

| Env | Default | Meaning |
|---|---|---|
| `PH_ML_LISTEN` | `0.0.0.0:3003` | Listen address |
| `PH_ML_CACHE_DIR` | `/cache` | Model cache directory (mount a volume here) |
| `PH_ML_API_KEY` | unset | Shared secret, expected in header `x-ml-key` |
| `PH_ML_CLIP_MODEL` | `ViT-B-32__openai` | Any `immich-app/<name>` CLIP repo on Hugging Face |
| `PH_ML_FACE_MODEL` | `buffalo_l` | Any `immich-app/<name>` InsightFace repo (`buffalo_l`, `buffalo_s`, `antelopev2`, ...) |
| `PH_ML_MODEL_TTL_SECS` | `300` | Unload models after this many idle seconds (`0` = never) |
| `PH_ML_THREADS` | `0` | Intra-op threads per ONNX session (`0` = ONNX Runtime default) |
| `PH_ML_PRELOAD` | unset | Comma list of `clip,faces,ocr,objects` to download and load at startup |
| `PH_ML_HF_ENDPOINT` | `https://huggingface.co` | Hugging Face base URL (mirror); used for the CLIP and face models |
| `PH_ML_OCR_DET_URL` | PP-OCRv4 mobile det (see below) | OCR text detection model (ONNX) |
| `PH_ML_OCR_REC_URL` | PP-OCRv5 latin mobile rec | OCR recognition model (ONNX) |
| `PH_ML_OCR_DICT_URL` | latin dict | Character dictionary matching the recognition model |
| `PH_ML_OCR_MAX_SIDE` | `960` | Longest image side fed to the OCR detector |
| `RUST_LOG` | `info,ort=warn` | Log filter |

Changing the CLIP model changes the embedding space, so the server has to re-run smart search for all assets. Likewise, changing the face model means faces must be re-detected.

## Models

Each file is downloaded once to `<cache>/<host>/<url path>`. The download goes to a `.part` file first and is renamed when complete. Concurrent requests share a single download and a single load.

| Task | Files (defaults) | License |
|---|---|---|
| CLIP | `https://huggingface.co/immich-app/ViT-B-32__openai/resolve/main/` + `visual/model.onnx`, `visual/preprocess_cfg.json`, `textual/model.onnx`, `textual/tokenizer.json`, `config.json` | MIT (OpenAI CLIP / OpenCLIP) |
| Faces | `https://huggingface.co/immich-app/buffalo_l/resolve/main/` + `detection/model.onnx` (SCRFD 10G), `recognition/model.onnx` (ArcFace R50) | **InsightFace: non-commercial research use only** |
| OCR detection | `https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv4/ch_PP-OCRv4_det_infer.onnx` | Apache-2.0 (PaddleOCR) |
| OCR recognition | `https://huggingface.co/monkt/paddleocr-onnx/resolve/main/languages/latin/rec.onnx` + `languages/latin/dict.txt` (PP-OCRv5 latin: English, German, French, Spanish and 28 more languages) | Apache-2.0 (PaddleOCR) |

On disk this takes about 810 MB, most of it CLIP (600 MB) and ArcFace (175 MB). With everything loaded, expect roughly 1.5 GB of RAM.

The buffalo_l models are licensed by InsightFace for non-commercial use only. Anyone using Pixelhort commercially must supply their own face model.

Other OCR models:

* For higher detection accuracy at about 10x the CPU time, use the PP-OCRv5 server detector: `PH_ML_OCR_DET_URL=https://huggingface.co/monkt/paddleocr-onnx/resolve/main/detection/v5/det.onnx`.
* Other scripts work with another `languages/<lang>/rec.onnx` and `dict.txt` pair from `monkt/paddleocr-onnx`, for example `english`, `chinese`, `korean`, `eslav`, `greek` or `thai`.

## Pipelines

* **CLIP image:** resize the shortest side to 224 (bicubic/Catmull-Rom), center crop, normalize with the mean/std from `preprocess_cfg.json`. The output is L2-normalized.
* **CLIP text:** tokenize with the HF tokenizer (BOS/EOS added), truncate to the context length from `config.json` (77) and zero-pad. The input type (int32 or int64) is read from the model. The output is L2-normalized.
* **Faces:** SCRFD detection.
  1. Letterbox to 640×640 (top-left aligned), normalize `(x-127.5)/128`.
  2. Decode strides 8/16/32 with 2 anchors each: score, distance bbox ×stride, 5 keypoints.
  3. Filter by `faceMinScore`, then NMS at 0.4.
  4. For each face, align with a least-squares similarity transform of the 5 landmarks onto the ArcFace 112×112 template (bilinear warp).
  5. Normalize `(x-127.5)/127.5`, run ArcFace, L2-normalize the 512-d output.
* **OCR:** PaddleOCR pipeline.
  1. Detection: max side 960, rounded to a multiple of 32, BGR, ImageNet normalization.
  2. DB post-processing: threshold 0.3, 8-connected components, mean-score filter at 0.6, convex hull, rotating-calipers minimum-area rectangle, unclip by `area·1.5/perimeter`.
  3. Rectify each line (vertical lines are rotated) and resize it to height 48 with width proportional (at least 320, zero-padded).
  4. CTC greedy decode with the dictionary. Lines with confidence below 0.5 are dropped. Lines are sorted in reading order.
* **Objects:** zero-shot classification over a built-in vocabulary of about 170 labels (`src/objects.rs`).
  1. Embed each label as "a photo of a {label}" with the CLIP text model, once per process.
  2. Compute `softmax(100·cos)` against the image embedding.
  3. Return the labels scoring at least 0.1, best 5 at most.

## Metrics

`GET /metrics` exposes:

* `ph_ml_requests_total{endpoint}`
* `ph_ml_tasks_total{task}`
* `ph_ml_errors_total{status}`
* `ph_ml_inference_seconds{task}` (summary)
* `ph_ml_model_loaded{model}`
* `ph_ml_download_bytes_total`

## Build and run

There is no local toolchain needed. Everything runs in Docker:

```sh
# compile / test (standalone cargo project, excluded from the root workspace)
PH_CRATE_DIR=crates/ml scripts/cargo.sh build --release
PH_CRATE_DIR=crates/ml scripts/cargo.sh test --release

# image (build context = repo root)
docker build -f docker/ml.Dockerfile -t pixelhort-ml .
docker run -d --name pixelhort-ml -p 3003:3003 -v pixelhort-ml-cache:/cache pixelhort-ml
```

ONNX Runtime is downloaded at build time by `ort` (`download-binaries`) and linked statically for CPU builds. The runtime image is `debian:bookworm-slim` plus the binary, about 180 MB.

### GPU (CUDA)

Build with the `cuda` cargo feature. This registers the CUDA execution provider, and `ort` falls back to the CPU if CUDA is unavailable. The CUDA build of ONNX Runtime is a shared library, which the Dockerfile copies next to the binary. It needs CUDA 12 and cuDNN 9 at runtime:

```sh
docker build -f docker/ml.Dockerfile --build-arg FEATURES=cuda \
  --build-arg RUNTIME_IMAGE=nvidia/cuda:12.8.1-cudnn-runtime-ubuntu22.04 -t pixelhort-ml:cuda .
docker run --gpus all -p 3003:3003 -v pixelhort-ml-cache:/cache pixelhort-ml:cuda
```

## Limitations

* Inference is not batched across requests, and each ONNX session is behind a mutex. Concurrent requests for the same model run one after another, while different models run in parallel. To scale, run more replicas.
* HEIC/AVIF/RAW input is not decoded. The server should send its JPEG/WebP preview.
* The object vocabulary is fixed and English-only. CLIP zero-shot scores are relative (softmax over the vocabulary), so an image showing something outside the vocabulary still gets the closest labels if they pass 0.1.
* The OCR detector produces axis-aligned or rotated rectangles, not curved polygons. There is no text-direction classifier, so text that is upside down (rotated 180°) is not recognized.
* The model files come from the `main` branch of their Hugging Face repos. They are not pinned to a revision and have no checksum verification.
