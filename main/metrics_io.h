#pragma once

#ifdef __cplusplus
extern "C" {
#endif

/** Start the USB serial reader. One JSON object per line updates the metrics model. */
void metrics_io_start(void);

#ifdef __cplusplus
}
#endif
