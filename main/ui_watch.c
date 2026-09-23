#include "ui_watch.h"

#include <stdio.h>
#include <string.h>

#include "esp_log.h"

LV_FONT_DECLARE(lv_font_montserrat_bold_12)
LV_FONT_DECLARE(lv_font_montserrat_bold_64)
#include "src/draw/lv_draw_line.h"
#include "src/draw/lv_draw_rect.h"

#define COLOR_BG 0x000000
#define COLOR_TEXT 0xFFFFFF
#define COLOR_TEXT_DIM 0x9A9A9C
#define COLOR_CYAN 0x3AE7ED
#define COLOR_TEMP_TRACK 0x0B2C30
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

#define USAGE_ARC_SIZE 216
#define TEMP_ARC_SIZE 178
#define ARC_WIDTH 13

static const char *TAG = "ui_watch";

typedef struct {
    lv_obj_t *screen;
    lv_obj_t *arc;
    lv_obj_t *usage_arc;
    lv_obj_t *warn;
    lv_obj_t *status;
    lv_obj_t *value;
    lv_obj_t *clock;
    lv_obj_t *watts;
    lv_obj_t *usage;
    lv_obj_t *rpm;
    lv_obj_t *memory;
    const char *memory_name;
    uint32_t accent;
} ui_temp_screen_t;

static ui_temp_screen_t s_cpu;
static ui_temp_screen_t s_gpu;

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

static void fan_draw_cb(lv_event_t *e)
{
    lv_obj_t *obj = lv_event_get_target(e);
    lv_layer_t *layer = lv_event_get_layer(e);
    lv_area_t area;
    lv_obj_get_coords(obj, &area);
    const int cx = (area.x1 + area.x2) / 2;
    const int cy = (area.y1 + area.y2) / 2;
    static const int blade[][2] = {{6, -3}, {3, 6}, {-6, 3}, {-3, -6}};

    for (int i = 0; i < 4; i++) {
        lv_draw_line_dsc_t dsc;
        lv_draw_line_dsc_init(&dsc);
        dsc.color = lv_color_hex(COLOR_TEXT);
        dsc.width = 2;
        dsc.round_start = 1;
        dsc.round_end = 1;
        dsc.opa = LV_OPA_COVER;
        dsc.p1.x = cx;
        dsc.p1.y = cy;
        dsc.p2.x = cx + blade[i][0];
        dsc.p2.y = cy + blade[i][1];
        lv_draw_line(layer, &dsc);
    }

    lv_draw_rect_dsc_t hub;
    lv_draw_rect_dsc_init(&hub);
    hub.bg_color = lv_color_hex(COLOR_TEXT);
    hub.bg_opa = LV_OPA_COVER;
    hub.radius = LV_RADIUS_CIRCLE;
    lv_area_t hub_area = {cx - 1, cy - 1, cx + 1, cy + 1};
    lv_draw_rect(layer, &hub, &hub_area);
}

static lv_obj_t *create_fan(lv_obj_t *parent)
{
    lv_obj_t *fan = lv_obj_create(parent);
    lv_obj_remove_style_all(fan);
    lv_obj_set_size(fan, 16, 16);
    lv_obj_clear_flag(fan, LV_OBJ_FLAG_CLICKABLE | LV_OBJ_FLAG_SCROLLABLE);
    lv_obj_add_event_cb(fan, fan_draw_cb, LV_EVENT_DRAW_MAIN, NULL);
    return fan;
}

static void create_temp_screen(ui_temp_screen_t *ui, lv_display_t *disp, uint32_t accent,
                               uint32_t track, const char *title, const char *memory_name)
{
    ui->accent = accent;
    ui->memory_name = memory_name;
    ui->screen = lv_display_get_screen_active(disp);
    style_screen_black(ui->screen);

    ui->usage_arc = lv_arc_create(ui->screen);
    style_arc(ui->usage_arc, USAGE_ARC_SIZE, accent, track);
    ui->arc = lv_arc_create(ui->screen);
    style_arc(ui->arc, TEMP_ARC_SIZE, COLOR_CYAN, COLOR_TEMP_TRACK);

    lv_obj_t *col = make_flex(ui->screen, LV_FLEX_FLOW_COLUMN);
    lv_obj_set_style_pad_row(col, 0, 0);
    lv_obj_align(col, LV_ALIGN_CENTER, 0, 2);

    lv_obj_t *title_row = make_flex(col, LV_FLEX_FLOW_ROW);
    lv_obj_set_style_pad_column(title_row, 4, 0);
    lv_obj_set_style_margin_bottom(title_row, 1, 0);
    ui->warn = create_text(title_row, LV_SYMBOL_WARNING, &lv_font_montserrat_12, accent);
    lv_obj_add_flag(ui->warn, LV_OBJ_FLAG_HIDDEN);
    ui->status = create_text(title_row, title, &lv_font_montserrat_bold_12, accent);
    lv_obj_set_style_text_letter_space(ui->status, 1, 0);

    ui->value = create_text(col, "—", &lv_font_montserrat_bold_64, COLOR_TEXT);
    lv_obj_set_style_margin_bottom(ui->value, 2, 0);

    lv_obj_t *clock_row = make_flex(col, LV_FLEX_FLOW_ROW);

    lv_obj_set_style_margin_top(clock_row, 4, 0);
    lv_obj_set_style_pad_column(clock_row, 8, 0);
    ui->clock = create_text(clock_row, "— GHz", &lv_font_montserrat_14, COLOR_TEXT_DIM);
    ui->watts = create_text(clock_row, "— W", &lv_font_montserrat_14, COLOR_TEXT_DIM);

    lv_obj_t *load_row = make_flex(col, LV_FLEX_FLOW_ROW);
    lv_obj_set_style_pad_column(load_row, 6, 0);
    lv_obj_set_style_margin_top(load_row, 2, 0);
    lv_obj_set_style_margin_bottom(load_row, 3, 0);
    ui->usage = create_text(load_row, "—%", &lv_font_montserrat_14, accent);
    create_fan(load_row);
    ui->rpm = create_text(load_row, "—", &lv_font_montserrat_14, COLOR_TEXT);

    char memory[24];
    snprintf(memory, sizeof(memory), "%s — / — GB", memory_name);
    ui->memory = create_text(col, memory, &lv_font_montserrat_10, COLOR_CYAN);
    lv_obj_add_flag(ui->memory, LV_OBJ_FLAG_HIDDEN);
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

static void show_placeholder(ui_temp_screen_t *ui)
{
    lv_arc_set_value(ui->arc, 0);
    lv_arc_set_value(ui->usage_arc, 0);
    lv_label_set_text(ui->value, "—");
    lv_obj_add_flag(ui->warn, LV_OBJ_FLAG_HIDDEN);
    lv_label_set_text(ui->clock, "— GHz");
    lv_label_set_text(ui->watts, "— W");
    lv_label_set_text(ui->usage, "—%");
    lv_label_set_text(ui->rpm, "—");
    char memory[24];
    snprintf(memory, sizeof(memory), "%s — / — GB", ui->memory_name);
    lv_label_set_text(ui->memory, memory);
    set_text_color(ui->status, COLOR_TEXT_DIM);
    set_text_color(ui->value, COLOR_TEXT_DIM);
    set_text_color(ui->usage, COLOR_TEXT_DIM);
}

static void update_temp_screen(ui_temp_screen_t *ui, const metrics_temp_t *temp, float temp_max,
                               metrics_ui_state_t state, int fan_rpm)
{
    if (state == METRICS_UI_WAITING || !temp->valid) {
        show_placeholder(ui);
        if (state == METRICS_UI_ERROR) {
            set_text_color(ui->status, COLOR_ERROR);
        }
        return;
    }

    char value[16];
    snprintf(value, sizeof(value), "%d°", (int) (temp->temp_c + 0.5f));
    lv_label_set_text(ui->value, value);

    char clock[16];
    snprintf(clock, sizeof(clock), "%.1f GHz", temp->clock_ghz);
    lv_label_set_text(ui->clock, clock);

    char watts[16];
    snprintf(watts, sizeof(watts), "%.0f W", temp->power_w);
    lv_label_set_text(ui->watts, watts);

    char usage[8];
    snprintf(usage, sizeof(usage), "%.0f%%", temp->usage_pct);
    lv_label_set_text(ui->usage, usage);

    char rpm[12];
    if (fan_rpm < 0) {
        lv_label_set_text(ui->rpm, "—");
    } else {
        snprintf(rpm, sizeof(rpm), "%d", fan_rpm);
        lv_label_set_text(ui->rpm, rpm);
    }

    char memory[32];
    snprintf(memory, sizeof(memory), "%s %.1f / %.0f GB", ui->memory_name, temp->memory_used_gb,
             temp->memory_total_gb);
    lv_label_set_text(ui->memory, memory);

    lv_arc_set_value(ui->arc, clamp_pct(temp->temp_c, temp_max));
    lv_arc_set_value(ui->usage_arc, clamp_pct(temp->usage_pct, 100.0f));

    /* From 80 C the label and temperature go orange, from 90 C red. Rings stay put. */
    uint32_t label_color = ui->accent;
    uint32_t value_color = COLOR_TEXT;
    bool warn = false;
    if (temp->temp_c >= TEMP_HOT_C) {
        label_color = COLOR_HOT;
        value_color = COLOR_HOT;
        warn = true;
    } else if (temp->temp_c >= TEMP_WARM_C) {
        label_color = COLOR_WARM;
        value_color = COLOR_WARM;
        warn = true;
    } else if (state == METRICS_UI_STALE) {
        label_color = COLOR_STALE;
        value_color = COLOR_TEXT_DIM;
    } else if (state == METRICS_UI_ERROR) {
        label_color = COLOR_ERROR;
    }

    set_text_color(ui->status, label_color);
    set_text_color(ui->value, value_color);
    set_text_color(ui->usage, ui->accent);
    set_text_color(ui->memory, COLOR_CYAN);
    if (warn) {
        set_text_color(ui->warn, label_color);
        lv_obj_remove_flag(ui->warn, LV_OBJ_FLAG_HIDDEN);
    } else {
        lv_obj_add_flag(ui->warn, LV_OBJ_FLAG_HIDDEN);
    }
}

void ui_watch_create(lv_display_t *disp_cpu, lv_display_t *disp_gpu)
{
    lv_display_set_default(disp_cpu);
    create_temp_screen(&s_cpu, disp_cpu, COLOR_USAGE_CPU, COLOR_TRACK_CPU, "CPU", "RAM");

    lv_display_set_default(disp_gpu);
    create_temp_screen(&s_gpu, disp_gpu, COLOR_USAGE_GPU, COLOR_TRACK_GPU, "GPU", "VRAM");
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
    update_temp_screen(&s_cpu, &snap->cpu, METRICS_CPU_TEMP_MAX_DEFAULT, snap->state,
                       fan_rpm_by_id(snap, "cpu"));
    update_temp_screen(&s_gpu, &snap->gpu, METRICS_GPU_TEMP_MAX_DEFAULT, snap->state,
                       fan_rpm_by_id(snap, "gpu"));
}
