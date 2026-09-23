#pragma once

#include "esp_err.h"
#include "metrics_model.h"

#ifdef __cplusplus
extern "C" {
#endif

/** Parse one compact JSON snapshot line into `out`. Does not touch the model. */
esp_err_t metrics_parse_line(const char *line, metrics_snapshot_t *out);

#ifdef __cplusplus
}
#endif
