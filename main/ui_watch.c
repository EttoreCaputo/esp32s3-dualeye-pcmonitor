#include "ui_watch.h"

#include <stdio.h>
#include <string.h>

#include "esp_log.h"

LV_FONT_DECLARE(lv_font_montserrat_bold_12)
LV_FONT_DECLARE(lv_font_montserrat_bold_48)
LV_FONT_DECLARE(lv_font_montserrat_bold_72)
LV_FONT_DECLARE(lv_font_fan_16)

/* Font Awesome 6 Free solid "fan", U+F863 */
#define UI_SYMBOL_FAN "\xEF\xA1\xA3"

/* Missing values: the large fonts carry an em dash, the built-in 14 px one is
 * ASCII only, so small labels use "--". */
#define COLOR_BG 0x000000
#define COLOR_TEXT 0xFFFFFF
#define COLOR_TEXT_DIM 0x9A9A9C
#define COLOR_CYAN 0x3AE7ED
#define COLOR_TEMP_TRACK 0x0B2C30
#define COLOR_MEM 0x5E8BFF
#define COLOR_MEM_TRACK 0x141D3A
#define COLOR_USAGE_CPU 0xC4F06A
#define COLOR_USAGE_GPU 0xC86CF0
#define COLOR_TRACK_CPU 0x163012
#define COLOR_TRACK_GPU 0x2A1238
#define COLOR_WARM 0xF8A639
#define COLOR_HOT 0xF05354
#define COLOR_ERROR 0xFF453A
#define COLOR_STALE 0xFFD60A

#define TEMP_WARM_C 80.0f
#define TEMP_HOT_C 90.0f
#define MEM_HIGH_PCT 90

#define USAGE_ARC_SIZE 216
#define RING_GAP 32
#define ARC_WIDTH 13
/* Gauge face: 270° sweep, open at the bottom. */
#define GAUGE_ROTATION 135
#define GAUGE_SWEEP 270

static const char *TAG = "ui_watch";

typedef struct {
    lv_obj_t *warn;
    lv_obj_t *label;
} ui_title_t;

/* Temperature, clock, power, load ring and fan. */
typedef struct {
    lv_obj_t *root;
    lv_obj_t *usage_arc;
    ui_title_t title;
    lv_obj_t *value;
    lv_obj_t *clock;
    lv_obj_t *watts;
    lv_obj_t *usage;
    lv_obj_t *rpm;
} ui_classic_t;

/* Activity-style rings, outside in: load, temperature, memory. */
typedef struct {
    lv_obj_t *root;
    lv_obj_t *usage_arc;
    lv_obj_t *temp_arc;
    lv_obj_t *mem_arc;
    ui_title_t title;
    lv_obj_t *value;
    lv_obj_t *usage;
    lv_obj_t *mem;
} ui_rings_t;

/* RAM or VRAM: used GiB in the middle, share of the total on the ring. */
typedef struct {
    lv_obj_t *root;
    lv_obj_t *arc;
    ui_title_t title;
    lv_obj_t *value;
    lv_obj_t *total;
    lv_obj_t *pct;
} ui_memory_t;

/* 270° temperature gauge around a large readout, load in the gap below. */
typedef struct {
    lv_obj_t *root;
    lv_obj_t *arc;
    ui_title_t title;
    lv_obj_t *value;
    lv_obj_t *usage;
} ui_gauge_t;

typedef struct {
    lv_obj_t *screen;
    const char *name;
    const char *mem_name;
    uint32_t accent;
    uint32_t track;
    ui_classic_t classic;
    ui_rings_t rings;
    ui_memory_t memory;
    ui_gauge_t gauge;
} ui_screen_t;

/* Label/value colours and the warning icon, from temperature and link state. */
typedef struct {
    uint32_t label;
    uint32_t value;
    bool warn;
} ui_tone_t;

static ui_screen_t s_cpu;
static ui_screen_t s_gpu;

static void style_screen_black(lv_obj_t *screen)
{
    lv_obj_set_style_bg_color(screen, lv_color_hex(COLOR_BG), 0);
    lv_obj_set_style_bg_opa(screen, LV_OPA_COVER, 0);
    lv_obj_clear_flag(screen, LV_OBJ_FLAG_SCROLLABLE);
}

static lv_obj_t *create_text(lv_obj_t *parent, const char *text, const lv_font_t *font, uint32_t color)
{
    lv_obj_t *label = lv_label_create(parent);
    lv_label_set_text(label, text);
    lv_obj_set_style_text_font(label, font, 0);
    lv_obj_set_style_text_color(label, lv_color_hex(color), 0);
    lv_obj_set_style_text_align(label, LV_TEXT_ALIGN_CENTER, 0);
    return label;
}

static void set_text_color(lv_obj_t *label, uint32_t color)
{
    lv_obj_set_style_text_color(label, lv_color_hex(color), 0);
}

static lv_obj_t *make_flex(lv_obj_t *parent, lv_flex_flow_t flow)
{
    lv_obj_t *obj = lv_obj_create(parent);
    lv_obj_remove_style_all(obj);
    lv_obj_set_size(obj, LV_SIZE_CONTENT, LV_SIZE_CONTENT);
    lv_obj_set_flex_flow(obj, flow);
    lv_obj_set_flex_align(obj, LV_FLEX_ALIGN_CENTER, LV_FLEX_ALIGN_CENTER, LV_FLEX_ALIGN_CENTER);
    lv_obj_clear_flag(obj, LV_OBJ_FLAG_SCROLLABLE);
    return obj;
}

/* A transparent full-screen layer holding one face. */
static lv_obj_t *make_face(lv_obj_t *screen)
{
    lv_obj_t *obj = lv_obj_create(screen);
    lv_obj_remove_style_all(obj);
    lv_obj_set_size(obj, LV_PCT(100), LV_PCT(100));
    lv_obj_clear_flag(obj, LV_OBJ_FLAG_SCROLLABLE | LV_OBJ_FLAG_CLICKABLE);
    lv_obj_add_flag(obj, LV_OBJ_FLAG_HIDDEN);
    return obj;
}

static void style_arc(lv_obj_t *arc, int size, uint32_t color, uint32_t track)
{
    lv_obj_set_size(arc, size, size);
    lv_obj_align(arc, LV_ALIGN_CENTER, 0, 0);
    lv_arc_set_rotation(arc, 270);
    lv_arc_set_bg_angles(arc, 0, 360);
    lv_arc_set_range(arc, 0, 100);
    lv_arc_set_value(arc, 0);
    lv_arc_set_mode(arc, LV_ARC_MODE_NORMAL);
    lv_obj_remove_style(arc, NULL, LV_PART_KNOB);
    lv_obj_clear_flag(arc, LV_OBJ_FLAG_CLICKABLE);

    lv_obj_set_style_arc_width(arc, ARC_WIDTH, LV_PART_MAIN);
    lv_obj_set_style_arc_color(arc, lv_color_hex(track), LV_PART_MAIN);
    lv_obj_set_style_arc_rounded(arc, true, LV_PART_MAIN);
    lv_obj_set_style_arc_width(arc, ARC_WIDTH, LV_PART_INDICATOR);
    lv_obj_set_style_arc_color(arc, lv_color_hex(color), LV_PART_INDICATOR);
    lv_obj_set_style_arc_rounded(arc, true, LV_PART_INDICATOR);
}

static lv_obj_t *create_arc(lv_obj_t *parent, int size, uint32_t color, uint32_t track)
{
    lv_obj_t *arc = lv_arc_create(parent);
    style_arc(arc, size, color, track);
    return arc;
}

static void set_arc_color(lv_obj_t *arc, uint32_t color)
{
    lv_obj_set_style_arc_color(arc, lv_color_hex(color), LV_PART_INDICATOR);
}

static lv_obj_t *create_fan(lv_obj_t *parent)
{
    return create_text(parent, UI_SYMBOL_FAN, &lv_font_fan_16, COLOR_TEXT);
}

static lv_obj_t *create_column(lv_obj_t *parent, int y_ofs)
{
    lv_obj_t *col = make_flex(parent, LV_FLEX_FLOW_COLUMN);
    lv_obj_set_style_pad_row(col, 0, 0);
    lv_obj_align(col, LV_ALIGN_CENTER, 0, y_ofs);
    return col;
}

static void create_title(ui_title_t *title, lv_obj_t *col, const char *text, uint32_t color, int margin_bottom)
{
    lv_obj_t *row = make_flex(col, LV_FLEX_FLOW_ROW);
    lv_obj_set_style_pad_column(row, 4, 0);
    lv_obj_set_style_margin_bottom(row, margin_bottom, 0);
    title->warn = create_text(row, LV_SYMBOL_WARNING, &lv_font_montserrat_12, color);
    lv_obj_add_flag(title->warn, LV_OBJ_FLAG_HIDDEN);
    title->label = create_text(row, text, &lv_font_montserrat_bold_12, color);
    lv_obj_set_style_text_letter_space(title->label, 1, 0);
}

static void set_title(ui_title_t *title, uint32_t color, bool warn)
{
    set_text_color(title->label, color);
    if (warn) {
        set_text_color(title->warn, color);
        lv_obj_remove_flag(title->warn, LV_OBJ_FLAG_HIDDEN);
    } else {
        lv_obj_add_flag(title->warn, LV_OBJ_FLAG_HIDDEN);
    }
}

static void create_classic(ui_screen_t *ui)
{
    ui_classic_t *f = &ui->classic;
    f->root = make_face(ui->screen);
    f->usage_arc = create_arc(f->root, USAGE_ARC_SIZE, ui->accent, ui->track);

    lv_obj_t *col = create_column(f->root, 2);
    create_title(&f->title, col, ui->name, ui->accent, 10);

    f->value = create_text(col, "—", &lv_font_montserrat_bold_48, COLOR_TEXT);
    lv_obj_set_style_margin_bottom(f->value, 2, 0);

    lv_obj_t *clock_row = make_flex(col, LV_FLEX_FLOW_ROW);
    lv_obj_set_style_margin_top(clock_row, 4, 0);
    lv_obj_set_style_pad_column(clock_row, 8, 0);
    f->clock = create_text(clock_row, "-- GHz", &lv_font_montserrat_14, COLOR_TEXT_DIM);
    f->watts = create_text(clock_row, "-- W", &lv_font_montserrat_14, COLOR_TEXT_DIM);

    lv_obj_t *load_row = make_flex(col, LV_FLEX_FLOW_ROW);
    lv_obj_set_style_pad_column(load_row, 6, 0);
    lv_obj_set_style_margin_top(load_row, 2, 0);
    lv_obj_set_style_margin_bottom(load_row, 3, 0);
    f->usage = create_text(load_row, "--%", &lv_font_montserrat_14, ui->accent);
    create_fan(load_row);
    f->rpm = create_text(load_row, "--", &lv_font_montserrat_14, COLOR_TEXT);
}

static void create_rings(ui_screen_t *ui)
{
    ui_rings_t *f = &ui->rings;
    f->root = make_face(ui->screen);
    f->usage_arc = create_arc(f->root, USAGE_ARC_SIZE, ui->accent, ui->track);
    f->temp_arc = create_arc(f->root, USAGE_ARC_SIZE - RING_GAP, COLOR_CYAN, COLOR_TEMP_TRACK);
    f->mem_arc = create_arc(f->root, USAGE_ARC_SIZE - 2 * RING_GAP, COLOR_MEM, COLOR_MEM_TRACK);

    lv_obj_t *col = create_column(f->root, 2);
    create_title(&f->title, col, ui->name, ui->accent, 6);
    f->value = create_text(col, "—", &lv_font_montserrat_bold_48, COLOR_TEXT);

    lv_obj_t *row = make_flex(col, LV_FLEX_FLOW_ROW);
    lv_obj_set_style_pad_column(row, 8, 0);
    lv_obj_set_style_margin_top(row, 6, 0);
    f->usage = create_text(row, "--%", &lv_font_montserrat_14, ui->accent);
    f->mem = create_text(row, "--%", &lv_font_montserrat_14, COLOR_MEM);
}

static void create_memory(ui_screen_t *ui)
{
    ui_memory_t *f = &ui->memory;
    f->root = make_face(ui->screen);
    f->arc = create_arc(f->root, USAGE_ARC_SIZE, COLOR_MEM, COLOR_MEM_TRACK);

    lv_obj_t *col = create_column(f->root, 2);
    create_title(&f->title, col, ui->mem_name, ui->accent, 10);
    f->value = create_text(col, "—", &lv_font_montserrat_bold_48, COLOR_TEXT);
    f->total = create_text(col, "of -- GB", &lv_font_montserrat_14, COLOR_TEXT_DIM);
    lv_obj_set_style_margin_top(f->total, 6, 0);
    f->pct = create_text(col, "--%", &lv_font_montserrat_14, COLOR_MEM);
    lv_obj_set_style_margin_top(f->pct, 2, 0);
}

static void create_gauge(ui_screen_t *ui)
{
    ui_gauge_t *f = &ui->gauge;
    f->root = make_face(ui->screen);
    f->arc = create_arc(f->root, USAGE_ARC_SIZE, COLOR_CYAN, COLOR_TEMP_TRACK);
    lv_arc_set_rotation(f->arc, GAUGE_ROTATION);
    lv_arc_set_bg_angles(f->arc, 0, GAUGE_SWEEP);

    lv_obj_t *col = create_column(f->root, -4);
    create_title(&f->title, col, ui->name, ui->accent, 8);
    f->value = create_text(col, "—", &lv_font_montserrat_bold_72, COLOR_TEXT);

    f->usage = create_text(f->root, "--%", &lv_font_montserrat_14, ui->accent);
    lv_obj_align(f->usage, LV_ALIGN_CENTER, 0, 84);
}

static void create_screen(ui_screen_t *ui, lv_display_t *disp, const char *name, const char *mem_name,
                          uint32_t accent, uint32_t track)
{
    ui->name = name;
    ui->mem_name = mem_name;
    ui->accent = accent;
    ui->track = track;
    ui->screen = lv_display_get_screen_active(disp);
    style_screen_black(ui->screen);

    create_classic(ui);
    create_rings(ui);
    create_memory(ui);
    create_gauge(ui);
    lv_obj_remove_flag(ui->classic.root, LV_OBJ_FLAG_HIDDEN);
}

static int clamp_pct(float value, float max_value)
{
    if (max_value <= 0.0f) {
        max_value = 100.0f;
    }
    int pct = (int) ((value / max_value) * 100.0f + 0.5f);
    if (pct < 0) {
        return 0;
    }
    return pct > 100 ? 100 : pct;
}

static int mem_pct(const metrics_temp_t *m)
{
    return m->mem_valid ? clamp_pct(m->mem_used_mb, m->mem_total_mb) : 0;
}

/* From 80 C the label and temperature go orange, from 90 C red. Rings stay put. */
static ui_tone_t temp_tone(const ui_screen_t *ui, float temp_c, metrics_ui_state_t state)
{
    ui_tone_t tone = {.label = ui->accent, .value = COLOR_TEXT, .warn = false};
    if (temp_c >= TEMP_HOT_C) {
        tone = (ui_tone_t) {.label = COLOR_HOT, .value = COLOR_HOT, .warn = true};
    } else if (temp_c >= TEMP_WARM_C) {
        tone = (ui_tone_t) {.label = COLOR_WARM, .value = COLOR_WARM, .warn = true};
    } else if (state == METRICS_UI_STALE) {
        tone = (ui_tone_t) {.label = COLOR_STALE, .value = COLOR_TEXT_DIM, .warn = false};
    } else if (state == METRICS_UI_ERROR) {
        tone.label = COLOR_ERROR;
    }
    return tone;
}

/* The temperature rings follow the same thresholds, cyan below them. */
static uint32_t temp_ring_color(float temp_c)
{
    if (temp_c >= TEMP_HOT_C) {
        return COLOR_HOT;
    }
    return temp_c >= TEMP_WARM_C ? COLOR_WARM : COLOR_CYAN;
}

static void set_temp_value(lv_obj_t *label, float temp_c)
{
    char value[16];
    snprintf(value, sizeof(value), "%d°", (int) (temp_c + 0.5f));
    lv_label_set_text(label, value);
}

static void set_pct(lv_obj_t *label, bool valid, int pct)
{
    if (!valid) {
        lv_label_set_text(label, "--%");
        return;
    }
    char text[8];
    snprintf(text, sizeof(text), "%d%%", pct);
    lv_label_set_text(label, text);
}

static uint32_t placeholder_title(metrics_ui_state_t state)
{
    return state == METRICS_UI_ERROR ? COLOR_ERROR : COLOR_TEXT_DIM;
}

static void update_classic(ui_screen_t *ui, const metrics_temp_t *temp, metrics_ui_state_t state, int fan_rpm)
{
    ui_classic_t *f = &ui->classic;
    if (state == METRICS_UI_WAITING || !temp->valid) {
        lv_arc_set_value(f->usage_arc, 0);
        lv_label_set_text(f->value, "—");
        lv_label_set_text(f->clock, "-- GHz");
        lv_label_set_text(f->watts, "-- W");
        lv_label_set_text(f->usage, "--%");
        lv_label_set_text(f->rpm, "--");
        set_title(&f->title, placeholder_title(state), false);
        set_text_color(f->value, COLOR_TEXT_DIM);
        set_text_color(f->usage, COLOR_TEXT_DIM);
        return;
    }

    set_temp_value(f->value, temp->temp_c);

    char clock[16];
    snprintf(clock, sizeof(clock), "%.1f GHz", temp->clock_ghz);
    lv_label_set_text(f->clock, clock);

    char watts[16];
    snprintf(watts, sizeof(watts), "%.0f W", temp->power_w);
    lv_label_set_text(f->watts, watts);

    char usage[8];
    snprintf(usage, sizeof(usage), "%.0f%%", temp->usage_pct);
    lv_label_set_text(f->usage, usage);

    if (fan_rpm < 0) {
        lv_label_set_text(f->rpm, "--");
    } else {
        char rpm[12];
        snprintf(rpm, sizeof(rpm), "%d", fan_rpm);
        lv_label_set_text(f->rpm, rpm);
    }

    lv_arc_set_value(f->usage_arc, clamp_pct(temp->usage_pct, 100.0f));

    ui_tone_t tone = temp_tone(ui, temp->temp_c, state);
    set_title(&f->title, tone.label, tone.warn);
    set_text_color(f->value, tone.value);
    set_text_color(f->usage, ui->accent);
}

static void update_rings(ui_screen_t *ui, const metrics_temp_t *temp, float temp_max, metrics_ui_state_t state)
{
    ui_rings_t *f = &ui->rings;
    if (state == METRICS_UI_WAITING || !temp->valid) {
        lv_arc_set_value(f->usage_arc, 0);
        lv_arc_set_value(f->temp_arc, 0);
        lv_arc_set_value(f->mem_arc, 0);
        lv_label_set_text(f->value, "—");
        lv_label_set_text(f->usage, "--%");
        lv_label_set_text(f->mem, "--%");
        set_title(&f->title, placeholder_title(state), false);
        set_text_color(f->value, COLOR_TEXT_DIM);
        set_text_color(f->usage, COLOR_TEXT_DIM);
        set_text_color(f->mem, COLOR_TEXT_DIM);
        return;
    }

    set_temp_value(f->value, temp->temp_c);
    int usage = clamp_pct(temp->usage_pct, 100.0f);
    set_pct(f->usage, true, usage);
    set_pct(f->mem, temp->mem_valid, mem_pct(temp));

    lv_arc_set_value(f->usage_arc, usage);
    lv_arc_set_value(f->temp_arc, clamp_pct(temp->temp_c, temp_max));
    lv_arc_set_value(f->mem_arc, mem_pct(temp));
    set_arc_color(f->temp_arc, temp_ring_color(temp->temp_c));

    ui_tone_t tone = temp_tone(ui, temp->temp_c, state);
    set_title(&f->title, tone.label, tone.warn);
    set_text_color(f->value, tone.value);
    set_text_color(f->usage, ui->accent);
    set_text_color(f->mem, temp->mem_valid ? COLOR_MEM : COLOR_TEXT_DIM);
}

static void update_memory(ui_screen_t *ui, const metrics_temp_t *temp, metrics_ui_state_t state)
{
    ui_memory_t *f = &ui->memory;
    if (state == METRICS_UI_WAITING || !temp->mem_valid) {
        lv_arc_set_value(f->arc, 0);
        lv_label_set_text(f->value, "—");
        lv_label_set_text(f->total, "of -- GB");
        lv_label_set_text(f->pct, "--%");
        set_title(&f->title, placeholder_title(state), false);
        set_text_color(f->value, COLOR_TEXT_DIM);
        set_text_color(f->pct, COLOR_TEXT_DIM);
        return;
    }

    char used[16];
    snprintf(used, sizeof(used), "%.1f", temp->mem_used_mb / 1024.0f);
    lv_label_set_text(f->value, used);

    char total[20];
    snprintf(total, sizeof(total), "of %.0f GB", temp->mem_total_mb / 1024.0f);
    lv_label_set_text(f->total, total);

    int pct = mem_pct(temp);
    set_pct(f->pct, true, pct);
    lv_arc_set_value(f->arc, pct);

    /* Nearly full memory goes orange like a warm temperature. */
    ui_tone_t tone = {.label = ui->accent, .value = COLOR_TEXT, .warn = false};
    if (pct >= MEM_HIGH_PCT) {
        tone = (ui_tone_t) {.label = COLOR_WARM, .value = COLOR_WARM, .warn = true};
    } else if (state == METRICS_UI_STALE) {
        tone = (ui_tone_t) {.label = COLOR_STALE, .value = COLOR_TEXT_DIM, .warn = false};
    } else if (state == METRICS_UI_ERROR) {
        tone.label = COLOR_ERROR;
    }
    set_title(&f->title, tone.label, tone.warn);
    set_text_color(f->value, tone.value);
    set_text_color(f->pct, COLOR_MEM);
}

static void update_gauge(ui_screen_t *ui, const metrics_temp_t *temp, float temp_max, metrics_ui_state_t state)
{
    ui_gauge_t *f = &ui->gauge;
    if (state == METRICS_UI_WAITING || !temp->valid) {
        lv_arc_set_value(f->arc, 0);
        lv_label_set_text(f->value, "—");
        lv_label_set_text(f->usage, "--%");
        set_title(&f->title, placeholder_title(state), false);
        set_text_color(f->value, COLOR_TEXT_DIM);
        set_text_color(f->usage, COLOR_TEXT_DIM);
        return;
    }

    set_temp_value(f->value, temp->temp_c);
    set_pct(f->usage, true, clamp_pct(temp->usage_pct, 100.0f));
    lv_arc_set_value(f->arc, clamp_pct(temp->temp_c, temp_max));
    set_arc_color(f->arc, temp_ring_color(temp->temp_c));

    ui_tone_t tone = temp_tone(ui, temp->temp_c, state);
    set_title(&f->title, tone.label, tone.warn);
    set_text_color(f->value, tone.value);
    set_text_color(f->usage, ui->accent);
}

static void show_face(ui_screen_t *ui, metrics_face_t face)
{
    lv_obj_t *roots[METRICS_FACE_COUNT] = {
        [METRICS_FACE_CLASSIC] = ui->classic.root,
        [METRICS_FACE_RINGS] = ui->rings.root,
        [METRICS_FACE_MEMORY] = ui->memory.root,
        [METRICS_FACE_GAUGE] = ui->gauge.root,
    };
    for (int i = 0; i < METRICS_FACE_COUNT; i++) {
        if (i == (int) face) {
            lv_obj_remove_flag(roots[i], LV_OBJ_FLAG_HIDDEN);
        } else {
            lv_obj_add_flag(roots[i], LV_OBJ_FLAG_HIDDEN);
        }
    }
}

/* Only the visible face is refreshed; a switch redraws it from the same snapshot. */
static void update_screen(ui_screen_t *ui, metrics_face_t face, const metrics_temp_t *temp, float temp_max,
                          metrics_ui_state_t state, int fan_rpm)
{
    if (face >= METRICS_FACE_COUNT) {
        face = METRICS_FACE_CLASSIC;
    }
    switch (face) {
    case METRICS_FACE_RINGS:
        update_rings(ui, temp, temp_max, state);
        break;
    case METRICS_FACE_MEMORY:
        update_memory(ui, temp, state);
        break;
    case METRICS_FACE_GAUGE:
        update_gauge(ui, temp, temp_max, state);
        break;
    default:
        update_classic(ui, temp, state, fan_rpm);
        break;
    }
    show_face(ui, face);
}

void ui_watch_create(lv_display_t *disp_cpu, lv_display_t *disp_gpu)
{
    lv_display_set_default(disp_cpu);
    create_screen(&s_cpu, disp_cpu, "CPU", "RAM", COLOR_USAGE_CPU, COLOR_TRACK_CPU);

    lv_display_set_default(disp_gpu);
    create_screen(&s_gpu, disp_gpu, "GPU", "VRAM", COLOR_USAGE_GPU, COLOR_TRACK_GPU);
    ESP_LOGI(TAG, "Watch UI created");
}

static int fan_rpm_by_id(const metrics_snapshot_t *snap, const char *id)
{
    for (size_t i = 0; i < snap->fan_count && i < METRICS_FAN_MAX; i++) {
        if (snap->fans[i].valid && strcmp(snap->fans[i].id, id) == 0) {
            return snap->fans[i].rpm;
        }
    }
    return -1;
}

void ui_watch_update(const metrics_snapshot_t *snap)
{
    if (snap == 0) {
        return;
    }
    update_screen(&s_cpu, snap->cpu_face, &snap->cpu, METRICS_CPU_TEMP_MAX_DEFAULT, snap->state,
                  fan_rpm_by_id(snap, "cpu"));
    update_screen(&s_gpu, snap->gpu_face, &snap->gpu, METRICS_GPU_TEMP_MAX_DEFAULT, snap->state,
                  fan_rpm_by_id(snap, "gpu"));
}
