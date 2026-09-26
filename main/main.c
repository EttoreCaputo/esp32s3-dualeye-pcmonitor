#include <string.h>

#include "board_display.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "lvgl_port.h"
#include "metrics_io.h"
#include "metrics_model.h"
#include "ui_watch.h"

static const char *TAG = "dualeye";

static void ui_refresh_task(void *arg)
{
    metrics_snapshot_t prev;
    memset(&prev, 0, sizeof(prev));

    while (true) {
        metrics_snapshot_t snap;
        metrics_model_get(&snap);
        if (memcmp(&prev, &snap, sizeof(snap)) != 0) {
            lvgl_port_lock();
            ui_watch_update(&snap);
            lvgl_port_unlock();
            prev = snap;
        }
        vTaskDelay(pdMS_TO_TICKS(200));
    }
}

void app_main(void)
{
    board_lcd_t lcds[BOARD_LCD_COUNT] = {0};
    ESP_ERROR_CHECK(board_display_init(lcds));

    metrics_model_init();

    lv_display_t *displays[BOARD_LCD_COUNT] = {0};
    ESP_ERROR_CHECK(lvgl_port_init(lcds, displays));

    lvgl_port_lock();
    ui_watch_create(displays[UI_SCREEN_CPU], displays[UI_SCREEN_GPU]);

    metrics_snapshot_t snap;
    metrics_model_get(&snap);
    ui_watch_update(&snap);
    lvgl_port_unlock();

    board_display_set_backlight(true);

    BaseType_t ui_ok = xTaskCreate(ui_refresh_task, "ui_refresh", 4096, NULL, 4, NULL);
    ESP_ERROR_CHECK(ui_ok == pdPASS ? ESP_OK : ESP_ERR_NO_MEM);
    metrics_io_start();
    // A host already listening (e.g. right after flashing) learns the version without asking.
    metrics_io_report_version();
    ESP_LOGI(TAG, "Watch UI ready, waiting for USB metrics");
}
