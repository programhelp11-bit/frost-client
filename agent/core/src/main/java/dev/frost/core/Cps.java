package dev.frost.core;

import java.util.ArrayDeque;

/** Sliding one-second click counter (thread safe). */
public final class Cps {
    private final ArrayDeque<Long> clicks = new ArrayDeque<>();

    public synchronized void click() {
        click(System.nanoTime());
    }

    public synchronized void click(long nowNanos) {
        clicks.addLast(nowNanos);
        prune(nowNanos);
    }

    public synchronized int count() {
        return count(System.nanoTime());
    }

    public synchronized int count(long nowNanos) {
        prune(nowNanos);
        return clicks.size();
    }

    private void prune(long now) {
        while (!clicks.isEmpty() && now - clicks.peekFirst() > 1_000_000_000L) {
            clicks.removeFirst();
        }
    }
}
