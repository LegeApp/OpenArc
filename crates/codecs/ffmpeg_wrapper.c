#include <stdint.h>
#include <stdlib.h>
#include <string.h>
 #include <stdio.h>

#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/avutil.h>
#include <libavutil/imgutils.h>
#include <libavutil/opt.h>
#include <libswscale/swscale.h>

 int openarc_ffmpeg_strerror(int err, char *buf, int buf_size) {
     if (!buf || buf_size <= 0) {
         return AVERROR(EINVAL);
     }
     buf[0] = '\0';
     return av_strerror(err, buf, (size_t)buf_size);
 }

static int open_decoder(AVFormatContext *in_fmt, int video_stream_index, AVCodecContext **dec_ctx_out) {
    AVStream *st = in_fmt->streams[video_stream_index];
    const AVCodec *dec = avcodec_find_decoder(st->codecpar->codec_id);
    if (!dec) {
        return AVERROR_DECODER_NOT_FOUND;
    }

    AVCodecContext *dec_ctx = avcodec_alloc_context3(dec);
    if (!dec_ctx) {
        return AVERROR(ENOMEM);
    }

    int ret = avcodec_parameters_to_context(dec_ctx, st->codecpar);
    if (ret < 0) {
        avcodec_free_context(&dec_ctx);
        return ret;
    }

    ret = avcodec_open2(dec_ctx, dec, NULL);
    if (ret < 0) {
        avcodec_free_context(&dec_ctx);
        return ret;
    }

    *dec_ctx_out = dec_ctx;
    return 0;
}

static int open_encoder(AVFormatContext *out_fmt, AVStream **out_stream_out, const char *encoder_name, int width, int height, AVRational time_base, AVRational framerate, const char *preset, int crf, AVCodecContext **enc_ctx_out) {
    const AVCodec *enc = avcodec_find_encoder_by_name(encoder_name);
    if (!enc) {
        return AVERROR_ENCODER_NOT_FOUND;
    }

    AVStream *out_st = avformat_new_stream(out_fmt, NULL);
    if (!out_st) {
        return AVERROR(ENOMEM);
    }

    AVCodecContext *enc_ctx = avcodec_alloc_context3(enc);
    if (!enc_ctx) {
        return AVERROR(ENOMEM);
    }

    enc_ctx->codec_id = enc->id;
    enc_ctx->codec_type = AVMEDIA_TYPE_VIDEO;
    enc_ctx->width = width;
    enc_ctx->height = height;
    enc_ctx->pix_fmt = AV_PIX_FMT_YUV420P;

    if (time_base.num > 0 && time_base.den > 0) {
        enc_ctx->time_base = time_base;
    } else if (framerate.num > 0 && framerate.den > 0) {
        enc_ctx->time_base = av_inv_q(framerate);
    } else {
        enc_ctx->time_base = (AVRational){1, 30};
    }

    if (framerate.num > 0 && framerate.den > 0) {
        enc_ctx->framerate = framerate;
    }

    if (out_fmt->oformat->flags & AVFMT_GLOBALHEADER) {
        enc_ctx->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
    }

    if (preset && preset[0] != '\0') {
        av_opt_set(enc_ctx->priv_data, "preset", preset, 0);
    }

    if (crf >= 0) {
        char crf_buf[16];
        snprintf(crf_buf, sizeof(crf_buf), "%d", crf);
        av_opt_set(enc_ctx->priv_data, "crf", crf_buf, 0);
    }

    int ret = avcodec_open2(enc_ctx, enc, NULL);
    if (ret < 0) {
        avcodec_free_context(&enc_ctx);
        return ret;
    }

    ret = avcodec_parameters_from_context(out_st->codecpar, enc_ctx);
    if (ret < 0) {
        avcodec_free_context(&enc_ctx);
        return ret;
    }

    out_st->time_base = enc_ctx->time_base;

    *out_stream_out = out_st;
    *enc_ctx_out = enc_ctx;
    return 0;
}

static int add_stream_copy(AVFormatContext *out_fmt, AVStream *in_st, AVStream **out_st_out) {
    AVStream *out_st = avformat_new_stream(out_fmt, NULL);
    if (!out_st) {
        return AVERROR(ENOMEM);
    }

    int ret = avcodec_parameters_copy(out_st->codecpar, in_st->codecpar);
    if (ret < 0) {
        return ret;
    }

    out_st->codecpar->codec_tag = 0;
    out_st->time_base = in_st->time_base;
    *out_st_out = out_st;
    return 0;
}

int openarc_ffmpeg_transcode(const char *input_path, const char *output_path, int codec, const char *preset, int crf, int copy_audio) {
    int ret = 0;
    AVFormatContext *in_fmt = NULL;
    AVFormatContext *out_fmt = NULL;

    AVCodecContext *dec_ctx = NULL;
    AVCodecContext *enc_ctx = NULL;

    AVStream *out_video_st = NULL;
    AVStream *out_audio_st = NULL;

    int video_stream_index = -1;
    int audio_stream_index = -1;

    struct SwsContext *sws = NULL;
    AVFrame *dec_frame = NULL;
    AVFrame *enc_frame = NULL;
    AVPacket *pkt = NULL;
    AVPacket *out_pkt = NULL;

    if (!input_path || !output_path) {
        return AVERROR(EINVAL);
    }

    ret = avformat_open_input(&in_fmt, input_path, NULL, NULL);
    if (ret < 0) {
        goto cleanup;
    }

    ret = avformat_find_stream_info(in_fmt, NULL);
    if (ret < 0) {
        goto cleanup;
    }

    for (unsigned int i = 0; i < in_fmt->nb_streams; i++) {
        AVStream *st = in_fmt->streams[i];
        if (video_stream_index < 0 && st->codecpar->codec_type == AVMEDIA_TYPE_VIDEO) {
            video_stream_index = (int)i;
        } else if (audio_stream_index < 0 && st->codecpar->codec_type == AVMEDIA_TYPE_AUDIO) {
            audio_stream_index = (int)i;
        }
    }

    if (video_stream_index < 0) {
        ret = AVERROR_STREAM_NOT_FOUND;
        goto cleanup;
    }

    ret = open_decoder(in_fmt, video_stream_index, &dec_ctx);
    if (ret < 0) {
        goto cleanup;
    }

    ret = avformat_alloc_output_context2(&out_fmt, NULL, NULL, output_path);
    if (ret < 0 || !out_fmt) {
        if (ret == 0) {
            ret = AVERROR_UNKNOWN;
        }
        goto cleanup;
    }

    AVStream *in_video_st = in_fmt->streams[video_stream_index];

    const char *enc_name = NULL;
    if (codec == 264) {
        enc_name = "libx264";
    } else if (codec == 265) {
        enc_name = "libx265";
    } else {
        ret = AVERROR(EINVAL);
        goto cleanup;
    }

    AVRational fr = in_video_st->r_frame_rate;
    if (fr.num == 0 || fr.den == 0) {
        fr = in_video_st->avg_frame_rate;
    }

    ret = open_encoder(
        out_fmt,
        &out_video_st,
        enc_name,
        dec_ctx->width,
        dec_ctx->height,
        in_video_st->time_base,
        fr,
        preset,
        crf,
        &enc_ctx
    );
    if (ret < 0) {
        goto cleanup;
    }

    if (copy_audio && audio_stream_index >= 0) {
        ret = add_stream_copy(out_fmt, in_fmt->streams[audio_stream_index], &out_audio_st);
        if (ret < 0) {
            goto cleanup;
        }
    }

    if (!(out_fmt->oformat->flags & AVFMT_NOFILE)) {
        ret = avio_open(&out_fmt->pb, output_path, AVIO_FLAG_WRITE);
        if (ret < 0) {
            goto cleanup;
        }
    }

    ret = avformat_write_header(out_fmt, NULL);
    if (ret < 0) {
        goto cleanup;
    }

    dec_frame = av_frame_alloc();
    enc_frame = av_frame_alloc();
    if (!dec_frame || !enc_frame) {
        ret = AVERROR(ENOMEM);
        goto cleanup;
    }

    enc_frame->format = enc_ctx->pix_fmt;
    enc_frame->width = enc_ctx->width;
    enc_frame->height = enc_ctx->height;

    ret = av_frame_get_buffer(enc_frame, 32);
    if (ret < 0) {
        goto cleanup;
    }

    if (dec_ctx->pix_fmt != enc_ctx->pix_fmt) {
        sws = sws_getContext(
            dec_ctx->width,
            dec_ctx->height,
            dec_ctx->pix_fmt,
            enc_ctx->width,
            enc_ctx->height,
            enc_ctx->pix_fmt,
            SWS_BILINEAR,
            NULL,
            NULL,
            NULL
        );
        if (!sws) {
            ret = AVERROR(EINVAL);
            goto cleanup;
        }
    }

    pkt = av_packet_alloc();
    out_pkt = av_packet_alloc();
    if (!pkt || !out_pkt) {
        ret = AVERROR(ENOMEM);
        goto cleanup;
    }

    while ((ret = av_read_frame(in_fmt, pkt)) >= 0) {
        if (pkt->stream_index == video_stream_index) {
            ret = avcodec_send_packet(dec_ctx, pkt);
            if (ret < 0) {
                break;
            }

            while ((ret = avcodec_receive_frame(dec_ctx, dec_frame)) >= 0) {
                AVFrame *frame_to_send = dec_frame;

                if (sws) {
                    ret = av_frame_make_writable(enc_frame);
                    if (ret < 0) {
                        break;
                    }

                    sws_scale(
                        sws,
                        (const uint8_t *const *)dec_frame->data,
                        dec_frame->linesize,
                        0,
                        dec_ctx->height,
                        enc_frame->data,
                        enc_frame->linesize
                    );

                    enc_frame->pts = av_rescale_q(dec_frame->pts, in_video_st->time_base, enc_ctx->time_base);
                    frame_to_send = enc_frame;
                } else {
                    dec_frame->pts = av_rescale_q(dec_frame->pts, in_video_st->time_base, enc_ctx->time_base);
                    frame_to_send = dec_frame;
                }

                ret = avcodec_send_frame(enc_ctx, frame_to_send);
                if (ret < 0) {
                    break;
                }

                while ((ret = avcodec_receive_packet(enc_ctx, out_pkt)) >= 0) {
                    out_pkt->stream_index = out_video_st->index;
                    av_packet_rescale_ts(out_pkt, enc_ctx->time_base, out_video_st->time_base);
                    ret = av_interleaved_write_frame(out_fmt, out_pkt);
                    av_packet_unref(out_pkt);
                    if (ret < 0) {
                        break;
                    }
                }

                if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
                    ret = 0;
                }

                av_frame_unref(dec_frame);
                if (ret < 0) {
                    break;
                }
            }

            if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
                ret = 0;
            }
        } else if (copy_audio && audio_stream_index >= 0 && pkt->stream_index == audio_stream_index && out_audio_st) {
            AVStream *in_audio_st = in_fmt->streams[audio_stream_index];
            pkt->stream_index = out_audio_st->index;
            av_packet_rescale_ts(pkt, in_audio_st->time_base, out_audio_st->time_base);
            ret = av_interleaved_write_frame(out_fmt, pkt);
            if (ret < 0) {
                break;
            }
        }

        av_packet_unref(pkt);
    }

    if (ret == AVERROR_EOF) {
        ret = 0;
    }

    if (ret < 0) {
        goto cleanup;
    }

    ret = avcodec_send_frame(enc_ctx, NULL);
    if (ret < 0) {
        goto cleanup;
    }

    while ((ret = avcodec_receive_packet(enc_ctx, out_pkt)) >= 0) {
        out_pkt->stream_index = out_video_st->index;
        av_packet_rescale_ts(out_pkt, enc_ctx->time_base, out_video_st->time_base);
        ret = av_interleaved_write_frame(out_fmt, out_pkt);
        av_packet_unref(out_pkt);
        if (ret < 0) {
            goto cleanup;
        }
    }

    if (ret == AVERROR_EOF || ret == AVERROR(EAGAIN)) {
        ret = 0;
    }

    if (ret < 0) {
        goto cleanup;
    }

    ret = av_write_trailer(out_fmt);

cleanup:
    if (pkt) {
        av_packet_free(&pkt);
    }
    if (out_pkt) {
        av_packet_free(&out_pkt);
    }
    if (sws) {
        sws_freeContext(sws);
        sws = NULL;
    }

    if (dec_frame) {
        av_frame_free(&dec_frame);
    }
    if (enc_frame) {
        av_frame_free(&enc_frame);
    }

    if (dec_ctx) {
        avcodec_free_context(&dec_ctx);
    }
    if (enc_ctx) {
        avcodec_free_context(&enc_ctx);
    }

    if (in_fmt) {
        avformat_close_input(&in_fmt);
    }

    if (out_fmt) {
        if (!(out_fmt->oformat->flags & AVFMT_NOFILE) && out_fmt->pb) {
            avio_closep(&out_fmt->pb);
        }
        avformat_free_context(out_fmt);
    }

    return ret;
}
