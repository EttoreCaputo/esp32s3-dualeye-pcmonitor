#pragma once

#ifdef __cplusplus
extern "C" {
#endif

/** The line the host sends to ask which firmware is running. */
#define METRICS_IO_VERSION_QUERY "?version"

/** Print `{"dualeye":"<version.txt>","idf":"<IDF version>"}` on its own line. */
void metrics_io_report_version(void);

/** Start the USB serial reader. One JSON object per line updates the metrics model;
 * METRICS_IO_VERSION_QUERY is answered with metrics_io_report_version(). */
void metrics_io_start(void);

#ifdef __cplusplus
}
#endif
