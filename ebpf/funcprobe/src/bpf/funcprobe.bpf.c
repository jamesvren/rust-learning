#include "vmlinux.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_tracing.h>

#include "funcprobe.h"

char LICENSE[] SEC("license") = "Dual BSD/GPL";

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 256 * 1024);
} events SEC(".maps");

int get_stack(struct pt_regs *ctx)
{
  struct task_struct *p = (void *) PT_REGS_PARM1_CORE(ctx);
  u32 pid = bpf_get_current_pid_tgid() >> 32;
  int cpu_id = bpf_get_smp_processor_id();
  struct stacktrace_event *event = bpf_ringbuf_reserve(&events, sizeof(*event), 0);

  if (!event) {
    bpf_printk("no ringbuf found!\n");
    return 1;
  }

  event->pid = pid;
  event->cpu_id = cpu_id;

  if (bpf_get_current_comm(event->comm, sizeof(event->comm)))
    event->comm[0] = 0;

  event->kstack_sz = bpf_get_stack(ctx, event->kstack, sizeof(event->kstack), 0);
  event->ustack_sz = bpf_get_stack(ctx, event->ustack, sizeof(event->ustack), BPF_F_USER_STACK);

  bpf_ringbuf_submit(event, 0);

  return 0;
}

//int BPF_KPROBE(kfunc_enter, struct pt_regs *ctx)
SEC("kprobe/kfunc")
int BPF_KPROBE(kfunc_enter)
{
  return get_stack(ctx);
}

SEC("kprobe/kfunc")
int BPF_KRETPROBE(kfunc_exit, int ret)
{
  return 0;
}

SEC("uprobe/ufunc")
int BPF_KPROBE(ufunc_enter)
{
  return get_stack(ctx);
}

SEC("uprobe/ufunc")
int BPF_KRETPROBE(ufunc_exit, int ret)
{
  return 0;
}
