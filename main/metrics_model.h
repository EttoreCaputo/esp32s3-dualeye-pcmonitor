#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define METRICS_FAN_MAX 3
#define METRICS_CPU_TEMP_MAX_DEFAULT 100.0f
#define METRICS_GPU_TEMP_MAX_DEFAULT 100.0f
#define METRICS_FAN_RPM_MAX_DEFAULT 3000
#define METRICS_STALE_MS_DEFAULT 3000

typedef enum {
    METRICS_UI_WAITING = 0,
    METRICS_UI_LIVE,
    METRICS_UI_STALE,
    METRICS_UI_ERROR,
} metrics_ui_state_t;

typedef struct {
    bool valid;
    float temp_c;
    float usage_pct;
    float clock_ghz;
    float power_w;
    float memory_used_gb;
    float memory_total_gb;
} metrics_temp_t;

typedef struct {
    char id[12];
    int rpm;
    bool valid;
} metrics_fan_t;

typedef struct {
    uint32_t ts;
    uint32_t updated_ms;
    metrics_temp_t cpu;
    metrics_temp_t gpu;
    metrics_fan_t fans[METRICS_FAN_MAX];
    size_t fan_count;
    metrics_ui_state_t state;
} metrics_snapshot_t;

void metrics_model_init(void);
void metrics_model_get(metrics_snapshot_t *out);
void metrics_model_set(const metrics_snapshot_t *in);

/** Phase 1: populate a fixed Watch-style demo snapshot. */
void metrics_model_load_mock(void);

#ifdef __cplusplus
}
#endif
