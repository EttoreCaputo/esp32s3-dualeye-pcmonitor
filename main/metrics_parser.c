#include "metrics_parser.h"

#include <ctype.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    const char *p;
    const char *end;
} js_t;

static void skip_ws(js_t *j)
{
    while (j->p < j->end && isspace((unsigned char) *j->p)) {
        j->p++;
    }
}

static bool consume(js_t *j, char c)
{
    skip_ws(j);
    if (j->p < j->end && *j->p == c) {
        j->p++;
        return true;
    }
    return false;
}

static bool expect_lit(js_t *j, const char *lit)
{
    size_t n = strlen(lit);
    if ((size_t) (j->end - j->p) < n || memcmp(j->p, lit, n) != 0) {
        return false;
    }
    j->p += n;
    return true;
}

static bool parse_string(js_t *j, char *out, size_t out_len)
{
    skip_ws(j);
    if (j->p >= j->end || *j->p != '"') {
        return false;
    }
    j->p++;
    size_t n = 0;
    while (j->p < j->end && *j->p != '"') {
        char c = *j->p++;
        if (c == '\\') {
            if (j->p >= j->end) {
                return false;
            }
            char e = *j->p++;
            if (e == 'n') {
                c = '\n';
            } else if (e == 't') {
                c = '\t';
            } else if (e == 'r') {
                c = '\r';
            } else {
                c = e;
            }
        }
        if (out != NULL && n + 1 < out_len) {
            out[n++] = c;
        }
    }
    if (j->p >= j->end || *j->p != '"') {
        return false;
    }
    j->p++;
    if (out != NULL && out_len > 0) {
        out[n] = '\0';
    }
    return true;
}

static bool parse_number(js_t *j, double *out)
{
    skip_ws(j);
    if (j->p >= j->end) {
        return false;
    }
    char *end = NULL;
    double value = strtod(j->p, &end);
    if (end == j->p) {
        return false;
    }
    j->p = end;
    *out = value;
    return true;
}

static bool skip_value(js_t *j, int depth)
{
    if (depth > 6) {
        return false;
    }
    skip_ws(j);
    if (j->p >= j->end) {
        return false;
    }
    char c = *j->p;
    if (c == '"') {
        return parse_string(j, NULL, 0);
    }
    if (c == '{') {
        j->p++;
        for (;;) {
            skip_ws(j);
            if (j->p < j->end && *j->p == '}') {
                j->p++;
                return true;
            }
            if (!parse_string(j, NULL, 0) || !consume(j, ':') || !skip_value(j, depth + 1)) {
                return false;
            }
            skip_ws(j);
            if (j->p < j->end && *j->p == ',') {
                j->p++;
            }
        }
    }
    if (c == '[') {
        j->p++;
        for (;;) {
            skip_ws(j);
            if (j->p < j->end && *j->p == ']') {
                j->p++;
                return true;
            }
            if (!skip_value(j, depth + 1)) {
                return false;
            }
            skip_ws(j);
            if (j->p < j->end && *j->p == ',') {
                j->p++;
            }
        }
    }
    if (c == 't') {
        return expect_lit(j, "true");
    }
    if (c == 'f') {
        return expect_lit(j, "false");
    }
    if (c == 'n') {
        return expect_lit(j, "null");
    }
    double ignored = 0.0;
    return parse_number(j, &ignored);
}

static bool object_key(js_t *j, char *key, size_t key_len, bool *done)
{
    skip_ws(j);
    if (j->p >= j->end) {
        return false;
    }
    if (*j->p == '}') {
        j->p++;
        *done = true;
        return true;
    }
    *done = false;
    if (!parse_string(j, key, key_len)) {
        return false;
    }
    return consume(j, ':');
}

static void object_sep(js_t *j)
{
    skip_ws(j);
    if (j->p < j->end && *j->p == ',') {
        j->p++;
    }
}

static bool read_number_field(js_t *j, float *out)
{
    double value = 0.0;
    if (!parse_number(j, &value)) {
        return false;
    }
    *out = (float) value;
    return true;
}

static bool parse_temp_object(js_t *j, metrics_temp_t *temp)
{
    if (!consume(j, '{')) {
        return false;
    }
    bool got_temp = false;
    for (;;) {
        char key[32];
        bool done = false;
        if (!object_key(j, key, sizeof(key), &done)) {
            return false;
        }
        if (done) {
            temp->valid = got_temp;
            return true;
        }
        if (strcmp(key, "temp_c") == 0) {
            if (!read_number_field(j, &temp->temp_c)) {
                return false;
            }
            got_temp = true;
        } else if (strcmp(key, "load_pct") == 0 || strcmp(key, "usage_pct") == 0) {
            if (!read_number_field(j, &temp->usage_pct)) {
                return false;
            }
        } else if (strcmp(key, "clock_mhz") == 0) {
            float mhz = 0.0f;
            if (!read_number_field(j, &mhz)) {
                return false;
            }
            temp->clock_ghz = mhz / 1000.0f;
        } else if (strcmp(key, "clock_ghz") == 0) {
            if (!read_number_field(j, &temp->clock_ghz)) {
                return false;
            }
        } else if (strcmp(key, "power_w") == 0) {
            if (!read_number_field(j, &temp->power_w)) {
                return false;
            }
        } else if (strcmp(key, "memory_used_gb") == 0) {
            if (!read_number_field(j, &temp->memory_used_gb)) {
                return false;
            }
        } else if (strcmp(key, "memory_total_gb") == 0) {
            if (!read_number_field(j, &temp->memory_total_gb)) {
                return false;
            }
        } else if (!skip_value(j, 1)) {
            return false;
        }
        object_sep(j);
    }
}

static void copy_fan_id(metrics_fan_t *fan, const char *id)
{
    if (id[0] == '\0') {
        return;
    }
    strncpy(fan->id, id, sizeof(fan->id) - 1);
    fan->id[sizeof(fan->id) - 1] = '\0';
}

static bool parse_fan_object(js_t *j, metrics_snapshot_t *snap)
{
    if (!consume(j, '{')) {
        return false;
    }
    char id[sizeof(snap->fans[0].id)] = {0};
    bool saw_rpm = false;
    int rpm = 0;
    for (;;) {
        char key[32];
        bool done = false;
        if (!object_key(j, key, sizeof(key), &done)) {
            return false;
        }
        if (done) {
            if (saw_rpm && snap->fan_count < METRICS_FAN_MAX) {
                metrics_fan_t *fan = &snap->fans[snap->fan_count++];
                memset(fan, 0, sizeof(*fan));
                copy_fan_id(fan, id);
                fan->rpm = rpm;
                fan->valid = true;
            }
            return true;
        }
        if (strcmp(key, "id") == 0) {
            if (!parse_string(j, id, sizeof(id))) {
                return false;
            }
        } else if (strcmp(key, "rpm") == 0) {
            double value = 0.0;
            if (!parse_number(j, &value)) {
                return false;
            }
            rpm = (int) (value >= 0.0 ? value + 0.5 : value - 0.5);
            saw_rpm = true;
        } else if (!skip_value(j, 1)) {
            return false;
        }
        object_sep(j);
    }
}

static bool parse_fans(js_t *j, metrics_snapshot_t *snap)
{
    if (!consume(j, '[')) {
        return false;
    }
    for (;;) {
        skip_ws(j);
        if (j->p >= j->end) {
            return false;
        }
        if (*j->p == ']') {
            j->p++;
            return true;
        }
        if (!parse_fan_object(j, snap)) {
            return false;
        }
        object_sep(j);
    }
}

esp_err_t metrics_parse_line(const char *line, metrics_snapshot_t *out)
{
    if (line == NULL || out == NULL) {
        return ESP_ERR_INVALID_ARG;
    }
    memset(out, 0, sizeof(*out));
    out->state = METRICS_UI_LIVE;

    js_t j = {.p = line, .end = line + strlen(line)};
    if (!consume(&j, '{')) {
        return ESP_FAIL;
    }

    for (;;) {
        char key[32];
        bool done = false;
        if (!object_key(&j, key, sizeof(key), &done)) {
            return ESP_FAIL;
        }
        if (done) {
            break;
        }
        bool ok = true;
        if (strcmp(key, "cpu") == 0) {
            ok = parse_temp_object(&j, &out->cpu);
        } else if (strcmp(key, "gpu") == 0) {
            ok = parse_temp_object(&j, &out->gpu);
        } else if (strcmp(key, "fans") == 0) {
            ok = parse_fans(&j, out);
        } else if (strcmp(key, "ts") == 0) {
            double value = 0.0;
            ok = parse_number(&j, &value);
            out->ts = (uint32_t) value;
        } else {
            ok = skip_value(&j, 1);
        }
        if (!ok) {
            return ESP_FAIL;
        }
        object_sep(&j);
    }

    if (!out->cpu.valid && !out->gpu.valid) {
        return ESP_ERR_INVALID_ARG;
    }
    return ESP_OK;
}
