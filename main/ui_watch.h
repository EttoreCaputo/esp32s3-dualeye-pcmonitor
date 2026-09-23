#pragma once

#include "lvgl.h"
#include "metrics_model.h"

#ifdef __cplusplus
extern "C" {
#endif

void ui_watch_create(lv_display_t *disp_cpu, lv_display_t *disp_gpu);
void ui_watch_update(const metrics_snapshot_t *snap);

#ifdef __cplusplus
}
#endif
