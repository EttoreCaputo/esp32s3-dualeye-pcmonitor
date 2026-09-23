#include "metrics_model.h"

#include <string.h>

#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"

static SemaphoreHandle_t s_lock;
static metrics_snapshot_t s_snapshot;

void metrics_model_init(void)
{
    s_lock = xSemaphoreCreateMutex();
    memset(&s_snapshot, 0, sizeof(s_snapshot));
    s_snapshot.state = METRICS_UI_WAITING;
}

void metrics_model_get(metrics_snapshot_t *out)
{
    if (out == NULL || s_lock == NULL) {
        return;
    }
    xSemaphoreTake(s_lock, portMAX_DELAY);
    *out = s_snapshot;
    xSemaphoreGive(s_lock);

    if (out->state == METRICS_UI_LIVE && out->updated_ms != 0) {
        uint32_t now_ms = (uint32_t) (esp_timer_get_time() / 1000);
        if ((uint32_t) (now_ms - out->updated_ms) > METRICS_STALE_MS_DEFAULT) {
            out->state = METRICS_UI_STALE;
        }
    }
}

void metrics_model_set(const metrics_snapshot_t *in)
{
    if (in == NULL || s_lock == NULL) {
        return;
    }
    metrics_snapshot_t copy = *in;
    copy.updated_ms = (uint32_t) (esp_timer_get_time() / 1000);
    xSemaphoreTake(s_lock, portMAX_DELAY);
    s_snapshot = copy;
    xSemaphoreGive(s_lock);
}

void metrics_model_load_mock(void)
{
    metrics_snapshot_t mock = {
        .ts = 1710000000,
        .cpu = {.valid = true,
                .temp_c = 62.0f,
                .usage_pct = 47.0f,
                .clock_ghz = 4.8f,
                .power_w = 65.0f,
                .memory_used_gb = 11.8f,
                .memory_total_gb = 32.0f},
        .gpu = {.valid = true,
                .temp_c = 58.0f,
                .usage_pct = 72.0f,
                .clock_ghz = 2.6f,
                .power_w = 210.0f,
                .memory_used_gb = 6.5f,
                .memory_total_gb = 12.0f},
        .fan_count = 3,
        .state = METRICS_UI_LIVE,
        .fans =
            {
                {.id = "cpu", .rpm = 1250, .valid = true},
                {.id = "gpu", .rpm = 2100, .valid = true},
                {.id = "sys", .rpm = 900, .valid = true},
            },
    };
    metrics_model_set(&mock);
}
