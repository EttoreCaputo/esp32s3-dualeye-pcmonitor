#pragma once

#include "board_display.h"
#include "esp_err.h"
#include "lvgl.h"

#ifdef __cplusplus
extern "C" {
#endif

esp_err_t lvgl_port_init(const board_lcd_t lcds[BOARD_LCD_COUNT],
                         lv_display_t *out_displays[BOARD_LCD_COUNT]);

void lvgl_port_lock(void);
void lvgl_port_unlock(void);

#ifdef __cplusplus
}
#endif
