/*******************************************************************************
 * Size: 16 px
 * Bpp: 4
 * Opts: --font /tmp/fa-solid-900.ttf -r 0xF863 --size 16 --bpp 4 --format lvgl --no-compress --no-prefilter --no-kerning --lv-font-name lv_font_fan_16 -o /Users/ec/Documents/PROGRAMMING/ESP-IDF/esp32s3-dualeye-pcmonitor/main/fonts/lv_font_fan_16.c
 ******************************************************************************/

#include "lvgl.h"

#ifndef LV_FONT_FAN_16
#define LV_FONT_FAN_16 1
#endif

#if LV_FONT_FAN_16

/*-----------------
 *    BITMAPS
 *----------------*/

/*Store the image of the glyphs*/
static LV_ATTRIBUTE_LARGE_CONST const uint8_t glyph_bitmap[] = {
    /* U+F863 "" */
    0x0, 0x0, 0x0, 0x6d, 0xc0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0xa, 0xff, 0xf0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x6f, 0xff, 0xd0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0xcf, 0xff, 0xb0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0xff, 0xff, 0xdb, 0xef, 0xc6, 0x0,
    0x0, 0x0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xa0,
    0x0, 0x0, 0xbf, 0xff, 0xff, 0xff, 0xff, 0xf6,
    0xcf, 0xdb, 0xdf, 0xf3, 0x3f, 0xff, 0xff, 0xfd,
    0xdf, 0xff, 0xff, 0xf3, 0x3f, 0xfd, 0xbd, 0xfc,
    0x6f, 0xff, 0xff, 0xff, 0xff, 0xfb, 0x0, 0x0,
    0xa, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0, 0x0,
    0x0, 0x6c, 0xff, 0xbd, 0xff, 0xff, 0x0, 0x0,
    0x0, 0x0, 0x0, 0xb, 0xff, 0xfc, 0x0, 0x0,
    0x0, 0x0, 0x0, 0xd, 0xff, 0xf6, 0x0, 0x0,
    0x0, 0x0, 0x0, 0xf, 0xff, 0xa0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0xc, 0xd7, 0x0, 0x0, 0x0
};


/*---------------------
 *  GLYPH DESCRIPTION
 *--------------------*/

static const lv_font_fmt_txt_glyph_dsc_t glyph_dsc[] = {
    {.bitmap_index = 0, .adv_w = 0, .box_w = 0, .box_h = 0, .ofs_x = 0, .ofs_y = 0} /* id = 0 reserved */,
    {.bitmap_index = 0, .adv_w = 256, .box_w = 16, .box_h = 16, .ofs_x = 0, .ofs_y = -2}
};

/*---------------------
 *  CHARACTER MAPPING
 *--------------------*/



/*Collect the unicode lists and glyph_id offsets*/
static const lv_font_fmt_txt_cmap_t cmaps[] =
{
    {
        .range_start = 63587, .range_length = 1, .glyph_id_start = 1,
        .unicode_list = NULL, .glyph_id_ofs_list = NULL, .list_length = 0, .type = LV_FONT_FMT_TXT_CMAP_FORMAT0_TINY
    }
};



/*--------------------
 *  ALL CUSTOM DATA
 *--------------------*/

#if LVGL_VERSION_MAJOR == 8
/*Store all the custom data of the font*/
static  lv_font_fmt_txt_glyph_cache_t cache;
#endif

#if LVGL_VERSION_MAJOR >= 8
static const lv_font_fmt_txt_dsc_t font_dsc = {
#else
static lv_font_fmt_txt_dsc_t font_dsc = {
#endif
    .glyph_bitmap = glyph_bitmap,
    .glyph_dsc = glyph_dsc,
    .cmaps = cmaps,
    .kern_dsc = NULL,
    .kern_scale = 0,
    .cmap_num = 1,
    .bpp = 4,
    .kern_classes = 0,
    .bitmap_format = 0,
#if LVGL_VERSION_MAJOR == 8
    .cache = &cache
#endif
};



/*-----------------
 *  PUBLIC FONT
 *----------------*/

/*Initialize a public general font descriptor*/
#if LVGL_VERSION_MAJOR >= 8
const lv_font_t lv_font_fan_16 = {
#else
lv_font_t lv_font_fan_16 = {
#endif
    .get_glyph_dsc = lv_font_get_glyph_dsc_fmt_txt,    /*Function pointer to get glyph's data*/
    .get_glyph_bitmap = lv_font_get_bitmap_fmt_txt,    /*Function pointer to get glyph's bitmap*/
    .line_height = 16,          /*The maximum line height required by the font*/
    .base_line = 2,             /*Baseline measured from the bottom of the line*/
#if !(LVGL_VERSION_MAJOR == 6 && LVGL_VERSION_MINOR == 0)
    .subpx = LV_FONT_SUBPX_NONE,
#endif
#if LV_VERSION_CHECK(7, 4, 0) || LVGL_VERSION_MAJOR >= 8
    .underline_position = -1,
    .underline_thickness = 1,
#endif
    .dsc = &font_dsc,          /*The custom font data. Will be accessed by `get_glyph_bitmap/dsc` */
#if LV_VERSION_CHECK(8, 2, 0) || LVGL_VERSION_MAJOR >= 9
    .fallback = NULL,
#endif
    .user_data = NULL,
};



#endif /*#if LV_FONT_FAN_16*/

