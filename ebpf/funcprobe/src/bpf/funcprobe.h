#ifndef __FUNCPROBE_H_
#define __FUNCPROBE_H_

#ifndef TASK_COMM_LEN
#define TASK_COMM_LEN 16
#endif

#ifndef MAX_STACK_DEPTH
#define MAX_STACK_DEPTH 128
#endif

typedef __u64 stack_strace_t[MAX_STACK_DEPTH];

struct stacktrace_event {
  __u32 pid;
  __u32 cpu_id;
  char comm[TASK_COMM_LEN];
  __u32 kstack_sz;
  __u32 ustack_sz;
  stack_strace_t kstack;
  stack_strace_t ustack;
};

#endif /* __FUNCPROBE_H_ */
