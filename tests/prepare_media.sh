#!/usr/bin/env bash
# Generates the e2e test media. Run inside the server runtime image (needs exiftool, ffmpeg, heif-enc):
#   docker run --rm -v "$PWD/testdata":/w pixelhort-runtime bash /w/prepare_media.sh
# Expects public-domain portraits in /w/test: photo.jpg (with a face and text), armstrong_variant.jpg, photo_exif6.jpg (photo.jpg with EXIF orientation 6), aldrin.jpg.
set -e
cd /w/media
q="-hide_banner -loglevel error -y"
cp ../test/photo.jpg armstrong.jpg
exiftool -q -overwrite_original -DateTimeOriginal="2019:10:26 14:30:00" -OffsetTimeOriginal="+02:00" \
  -GPSLatitude=52.5163 -GPSLatitudeRef=N -GPSLongitude=13.3777 -GPSLongitudeRef=E -Make=NASA -Model="Hasselblad 500C" \
  -FNumber=5.6 -ExposureTime=0.004 -ISO=100 -Keywords=Mond -Keywords="Raumfahrt|Apollo" armstrong.jpg
cp ../test/armstrong_variant.jpg armstrong2.jpg
exiftool -q -overwrite_original -DateTimeOriginal="2020:07:20 10:00:00" -GPSLatitude=48.137 -GPSLatitudeRef=N -GPSLongitude=11.575 -GPSLongitudeRef=E armstrong2.jpg
cp ../test/photo_exif6.jpg armstrong3.jpg
exiftool -q -overwrite_original -DateTimeOriginal="2021:03:01 09:00:00" armstrong3.jpg
cp ../test/aldrin.jpg aldrin.jpg
exiftool -q -overwrite_original -DateTimeOriginal="2018:10:26 08:00:00" -GPSLatitude=-33.8688 -GPSLatitudeRef=S -GPSLongitude=151.2093 -GPSLongitudeRef=E aldrin.jpg
ffmpeg $q -f lavfi -i "testsrc2=size=2000x1000" -frames:v 1 pano360.jpg
exiftool -q -overwrite_original -XMP-GPano:ProjectionType=equirectangular -XMP-GPano:UsePanoramaViewer=True -DateTimeOriginal="2022:06:01 12:00:00" pano360.jpg
ffmpeg $q -f lavfi -i "testsrc2=size=1280x720:rate=30" -f lavfi -i "sine=frequency=440" -t 3 -c:v libx265 -tag:v hvc1 -c:a aac clip.mov
heif-enc -q 80 aldrin.jpg -o live.heic
exiftool -q -overwrite_original -DateTimeOriginal="2023:05:05 18:00:00" live.heic || true
ffmpeg $q -f lavfi -i "testsrc=size=640x480:rate=30" -t 2 -c:v libx264 -pix_fmt yuv420p live.mov
ffmpeg $q -f lavfi -i "testsrc=size=640x480:rate=30" -t 2 -c:v libx264 -pix_fmt yuv420p mv.mp4
cp ../test/aldrin.jpg mvtmp.jpg
exiftool -q -overwrite_original -XMP-GCamera:MicroVideo=1 -XMP-GCamera:MicroVideoVersion=1 -XMP-GCamera:MicroVideoOffset=$(stat -c%s mv.mp4) -DateTimeOriginal="2024:01:01 00:00:00" mvtmp.jpg
cat mvtmp.jpg mv.mp4 > MVIMG_20240101.jpg; rm mvtmp.jpg mv.mp4
cd /w/ext
ffmpeg $q -f lavfi -i "testsrc2=size=800x600" -frames:v 1 library1.jpg
ffmpeg $q -f lavfi -i "mandelbrot=size=640x480" -frames:v 1 sub/library2.png
ls -la /w/media /w/ext /w/ext/sub
