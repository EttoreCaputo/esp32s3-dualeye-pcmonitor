/*******************************************************************************
 * Size: 12 px
 * Bpp: 4
 * Opts: --font /tmp/Montserrat-Bold.ttf --size 12 --bpp 4 --format lvgl --no-compress --no-prefilter --no-kerning --lv-font-name lv_font_montserrat_bold_12 --symbols CPUG -o main/fonts/lv_font_montserrat_bold_12.c
 ******************************************************************************/

#include "lvgl.h"

#ifndef LV_FONT_MONTSERRAT_BOLD_12
#define LV_FONT_MONTSERRAT_BOLD_12 1
#endif

#if LV_FONT_MONTSERRAT_BOLD_12

/*-----------------
 *    BITMAPS
 *----------------*/

/*Store the image of the glyphs*/
static LV_ATTRIBUTE_LARGE_CONST const uint8_t glyph_bitmap[] = {
    /* U+0043 "C" */
    0x0, 0x3b, 0xff, 0xb4, 0x0, 0x6f, 0xfd, 0xdf,
    0xf3, 0x1f, 0xf5, 0x0, 0x37, 0x6, 0xfa, 0x0,
    0x0, 0x0, 0x8f, 0x70, 0x0, 0x0, 0x6, 0xfa,
    0x0, 0x0, 0x0, 0x1f, 0xf5, 0x0, 0x37, 0x0,
    0x6f, 0xfd, 0xdf, 0xf3, 0x0, 0x3b, 0xff, 0xc4,
    0x0,

    /* U+0047 "G" */
    0x0, 0x3b, 0xff, 0xc5, 0x0, 0x6f, 0xfe, 0xdf,
    0xf5, 0x1f, 0xf5, 0x0, 0x28, 0x6, 0xfa, 0x0,
    0x0, 0x0, 0x8f, 0x70, 0x0, 0x3a, 0x46, 0xfa,
    0x0, 0x5, 0xf7, 0x1f, 0xf5, 0x0, 0x5f, 0x70,
    0x6f, 0xfd, 0xcf, 0xf7, 0x0, 0x3b, 0xff, 0xc6,
    0x0,

    /* U+0050 "P" */
    0xff, 0xff, 0xe9, 0x10, 0xff, 0xbb, 0xef, 0xb0,
    0xff, 0x0, 0xe, 0xf2, 0xff, 0x0, 0xb, 0xf3,
    0xff, 0x0, 0x3f, 0xf1, 0xff, 0xff, 0xff, 0x80,
    0xff, 0xbb, 0x94, 0x0, 0xff, 0x0, 0x0, 0x0,
    0xff, 0x0, 0x0, 0x0,

    /* U+0055 "U" */
    0x1f, 0xe0, 0x0, 0x6f, 0x81, 0xfe, 0x0, 0x6,
    0xf8, 0x1f, 0xe0, 0x0, 0x6f, 0x81, 0xfe, 0x0,
    0x6, 0xf8, 0x1f, 0xe0, 0x0, 0x6f, 0x80, 0xff,
    0x0, 0x7, 0xf7, 0xd, 0xf5, 0x0, 0xcf, 0x40,
    0x5f, 0xfd, 0xef, 0xd0, 0x0, 0x6d, 0xfe, 0xa1,
    0x0
};


/*---------------------
 *  GLYPH DESCRIPTION
 *--------------------*/

static const lv_font_fmt_txt_glyph_dsc_t glyph_dsc[] = {
    {.bitmap_index = 0, .adv_w = 0, .box_w = 0, .box_h = 0, .ofs_x = 0, .ofs_y = 0} /* id = 0 reserved */,
    {.bitmap_index = 0, .adv_w = 139, .box_w = 9, .box_h = 9, .ofs_x = 0, .ofs_y = 0},
    {.bitmap_index = 41, .adv_w = 148, .box_w = 9, .box_h = 9, .ofs_x = 0, .ofs_y = 0},
    {.bitmap_index = 82, .adv_w = 141, .box_w = 8, .box_h = 9, .ofs_x = 1, .ofs_y = 0},
    {.bitmap_index = 118, .adv_w = 151, .box_w = 9, .box_h = 9, .ofs_x = 0, .ofs_y = 0}
};

/*---------------------
 *  CHARACTER MAPPING
 *--------------------*/

static const uint16_t unicode_list_0[] = {
    0x0, 0x4, 0xd, 0x12
};

/*Collect the unicode lists and glyph_id offsets*/
static const lv_font_fmt_txt_cmap_t cmaps[] =
{
    {
        .range_start = 67, .range_length = 19, .glyph_id_start = 1,
        .unicode_list = unicode_list_0, .glyph_id_ofs_list = NULL, .list_length = 4, .type = LV_FONT_FMT_TXT_CMAP_SPARSE_TINY
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
const lv_font_t lv_font_montserrat_bold_12 = {
#else
lv_font_t lv_font_montserrat_bold_12 = {
#endif
    .get_glyph_dsc = lv_font_get_glyph_dsc_fmt_txt,    /*Function pointer to get glyph's data*/
    .get_glyph_bitmap = lv_font_get_bitmap_fmt_txt,    /*Function pointer to get glyph's bitmap*/
    .line_height = 9,          /*The maximum line height required by the font*/
    .base_line = 0,             /*Baseline measured from the bottom of the line*/
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



#endif /*#if LV_FONT_MONTSERRAT_BOLD_12*/

