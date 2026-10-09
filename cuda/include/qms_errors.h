#ifndef QMS_ERRORS_H
#define QMS_ERRORS_H

#define QMS_OK 0
#define QMS_ERR_LIBRARY (-1)
#define QMS_ERR_DEVICE (-2)
#define QMS_ERR_MEMORY (-3)
#define QMS_ERR_LAUNCH (-4)
#define QMS_ERR_ARGUMENT (-5)
#define QMS_ERR_HASH (-6)
#define QMS_ERR_UNSUPPORTED (-7)

#ifdef __cplusplus
extern "C" {
#endif

const char *qms_error_message(int32_t code);

#ifdef __cplusplus
}
#endif

#endif
