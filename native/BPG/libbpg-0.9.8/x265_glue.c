/*
 * x265 encoder front-end  
 *
 * Copyright (c) 2014 Fabrice Bellard
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
 * THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
 * THE SOFTWARE.
 */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <inttypes.h>
#include <unistd.h>

#include "bpgenc.h"

#include "x265.h"

struct HEVCEncoderContext {
    const x265_api *api;
    x265_encoder *enc;
    x265_picture *pic;
    uint8_t *buf;
    int buf_len, buf_size;
};

static int x265_apply_param(HEVCEncoderContext *s, x265_param *p,
                            const char *name, const char *value)
{
    if (s->api->param_parse(p, name, value) != 0) {
        fprintf(stderr, "x265: invalid parameter override %s=%s\n", name, value);
        return -1;
    }
    return 0;
}

static int x265_apply_param_list(HEVCEncoderContext *s, x265_param *p,
                                 const char *list)
{
    char *copy, *tok, *eq;
    int ret = 0;

    if (!list || !list[0])
        return 0;
    copy = strdup(list);
    if (!copy)
        return -1;
    for (tok = strtok(copy, ",;"); tok; tok = strtok(NULL, ",;")) {
        while (*tok == ' ' || *tok == '\t')
            tok++;
        if (!tok[0])
            continue;
        eq = strchr(tok, '=');
        if (!eq || eq == tok || !eq[1]) {
            fprintf(stderr, "x265: expected name=value in BPG_X265_PARAMS token '%s'\n", tok);
            ret = -1;
            break;
        }
        *eq = '\0';
        if (x265_apply_param(s, p, tok, eq + 1) < 0) {
            ret = -1;
            break;
        }
    }
    free(copy);
    return ret;
}

static void x265_apply_openarc_aq(x265_param *p, const HEVCEncodeParams *params)
{
    switch (params->aq_mode) {
    case 0:
        p->rc.aqMode = X265_AQ_NONE;
        p->rc.aqStrength = 0.0;
        break;
    case 1:
        p->rc.aqMode = X265_AQ_VARIANCE;
        if (params->aq_strength > 0.0f)
            p->rc.aqStrength = params->aq_strength;
        break;
    case 3:
        p->rc.aqMode = X265_AQ_AUTO_VARIANCE_BIASED;
        if (params->aq_strength > 0.0f)
            p->rc.aqStrength = params->aq_strength;
        break;
    case 6:
        /* bpg-rs has an experimental two-pass measured AQ mode.  The C/x265
         * production path maps it to strong auto-variance AQ rather than
         * silently disabling perceptual AQ. */
        p->rc.aqMode = X265_AQ_AUTO_VARIANCE_BIASED;
        p->rc.aqStrength = params->aq_strength > 0.0f ? params->aq_strength : 1.2;
        break;
    case 2:
    default:
        p->rc.aqMode = X265_AQ_AUTO_VARIANCE;
        if (params->aq_strength > 0.0f)
            p->rc.aqStrength = params->aq_strength;
        break;
    }
}

static HEVCEncoderContext *x265_open(const HEVCEncodeParams *params)
{
    HEVCEncoderContext *s;
    x265_param *p;
    int preset_index;
    const char *preset;
    
    s = malloc(sizeof(HEVCEncoderContext));
    memset(s, 0, sizeof(*s));

    s->api = x265_api_get(params->bit_depth);
    if (!s->api) {
        fprintf(stderr, "x265 supports bit depths of 8, 10 or 12.\n");
        return NULL;
    }
#if 0
    /* Note: the x265 library included in libbpg supported gray encoding */
    if (params->chroma_format == BPG_FORMAT_GRAY) {
        fprintf(stderr, "x265 does not support monochrome (or alpha) data yet. Plase use the jctvc encoder.\n");
        return NULL;
    }
#endif
    
    p = s->api->param_alloc();

    preset_index = params->compress_level; /* 9 is placebo */

    preset = x265_preset_names[preset_index];
    if (params->verbose)
        printf("Using x265 preset: %s\n", preset);
    
    s->api->param_default_preset(p, preset, "ssim");

    p->bRepeatHeaders = 1;
    p->decodedPictureHashSEI = params->sei_decoded_picture_hash;
    p->sourceWidth = params->width;
    p->sourceHeight = params->height;
    switch(params->chroma_format) {
    case BPG_FORMAT_GRAY:
        p->internalCsp = X265_CSP_I400;
        break;
    case BPG_FORMAT_420:
        p->internalCsp = X265_CSP_I420;
        break;
    case BPG_FORMAT_422:
        p->internalCsp = X265_CSP_I422;
        break;
    case BPG_FORMAT_444:
        p->internalCsp = X265_CSP_I444;
        break;
    default:
        abort();
    }
    if (params->intra_only) {
        p->keyframeMax = 1; /* only I frames */
        p->totalFrames = 1;
    } else {
        p->keyframeMax = 250;
        p->totalFrames = 0;
        p->maxNumReferences = 1;
        p->bframes = 0;
    }
    p->bEnableRectInter = 1;
    p->bEnableAMP = 1; /* cannot use 0 due to header restriction */
    p->internalBitDepth = params->bit_depth;
    p->bEmitInfoSEI = 0;
    if (params->verbose)
        p->logLevel = X265_LOG_INFO;
    else
        p->logLevel = X265_LOG_NONE;
        
    /* dummy frame rate */
    p->fpsNum = 25;
    p->fpsDenom = 1;

    /* Keep BPG's QP-style quality selection, but do not let x265's CQP
     * validation path erase the SSIM tune's AQ settings.  The vendored x265
     * 4.1 tree carries an OpenArc patch that permits AQ in CQP mode. */
    p->rc.rateControlMode = X265_RC_CQP;
    p->rc.qp = params->qp;
    x265_apply_openarc_aq(p, params);
    p->bLossless = params->lossless;

    if (getenv("BPG_X265_SINGLE_THREAD")) {
        if (x265_apply_param(s, p, "frame-threads", "1") < 0 ||
            x265_apply_param(s, p, "pools", "none") < 0)
            goto fail;
    }
    if (x265_apply_param_list(s, p, getenv("BPG_X265_PARAMS")) < 0)
        goto fail;

    s->enc = s->api->encoder_open(p);
    if (!s->enc)
        goto fail;

    s->pic = s->api->picture_alloc();
    s->api->picture_init(p, s->pic);

    s->pic->colorSpace = p->internalCsp;

    s->api->param_free(p);

    return s;

 fail:
    s->api->param_free(p);
    free(s);
    return NULL;
}

static void add_nal(HEVCEncoderContext *s, const uint8_t *data, int data_len)
{
    int new_size, size;

    size = s->buf_len + data_len;
    if (size > s->buf_size) {
        new_size = (s->buf_size * 3) / 2;
        if (new_size < size)
            new_size = size;
        s->buf = realloc(s->buf, new_size);
        s->buf_size = new_size;
    }
    memcpy(s->buf + s->buf_len, data, data_len);
    s->buf_len += data_len;
}

static int x265_encode(HEVCEncoderContext *s, Image *img)
{
    int c_count, i, ret;
    x265_picture *pic;
    uint32_t nal_count;
    x265_nal *p_nal;
    
    pic = s->pic;

    if (img->format == BPG_FORMAT_GRAY)
        c_count = 1;
    else
        c_count = 3;
    for(i = 0; i < c_count; i++) {
        pic->planes[i] = img->data[i];
        pic->stride[i] = img->linesize[i];
    }
    pic->bitDepth = img->bit_depth;

    ret = s->api->encoder_encode(s->enc, &p_nal, &nal_count, pic, NULL);
    if (ret > 0) {
        for(i = 0; i < nal_count; i++) {
            add_nal(s, p_nal[i].payload, p_nal[i].sizeBytes);
        }
    }
    return 0;
}

static int x265_close(HEVCEncoderContext *s, uint8_t **pbuf)
{
    int buf_len, ret, i;
    uint32_t nal_count;
    x265_nal *p_nal;
    
    /* get last compressed pictures */
    for(;;) {
        ret = s->api->encoder_encode(s->enc, &p_nal, &nal_count, NULL, NULL);
        if (ret <= 0)
            break;
        for(i = 0; i < nal_count; i++) {
            add_nal(s, p_nal[i].payload, p_nal[i].sizeBytes);
        }
    }

    if (s->buf_len < s->buf_size) {
        s->buf = realloc(s->buf, s->buf_len);
    }

    *pbuf = s->buf;
    buf_len = s->buf_len;

    s->api->encoder_close(s->enc);
    s->api->picture_free(s->pic);
    free(s);
    return buf_len;
}

HEVCEncoder x265_hevc_encoder = {
  .open = x265_open,
  .encode = x265_encode,
  .close = x265_close,
};
