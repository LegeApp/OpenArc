OpenARC is a tool meant for backing up phone media, what it does differently is compresses all images to BPG format which is a better version of the HEIC codec. A 2.5mb JPEG can be compressed down to 200kb with no loss in quality. 

Also, videos are compressed with ffmpeg with either H264 or H265. This is all automatic and the resulting archive is a mix of ARC and ZSTD depending on if there are non-media files or not. 

Android and Iphones have very poor compression in their camera apps so if you take a lot of pictures and videos, file size can balloon and if you want to back up your media, this is the way to do it. I made this tool after doing all of these steps individually, and now it's all automated.
