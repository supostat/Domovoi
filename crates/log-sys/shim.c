#include <os/log.h>

os_log_t domovoi_log_create(const char *subsystem, const char *category) {
    return os_log_create(subsystem, category);
}

void domovoi_log_release(os_log_t log) {
    os_release(log);
}

void domovoi_log_notice(os_log_t log, const char *message) {
    os_log_with_type(log, OS_LOG_TYPE_DEFAULT, "%{public}s", message);
}

void domovoi_log_error(os_log_t log, const char *message) {
    os_log_with_type(log, OS_LOG_TYPE_ERROR, "%{public}s", message);
}
