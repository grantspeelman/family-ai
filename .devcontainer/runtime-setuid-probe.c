/* Test fixture: report effective UID without performing privileged operations. */
#include <stdio.h>
#include <unistd.h>

int main(void) {
    printf("%lu\n", (unsigned long)geteuid());
    return 0;
}
