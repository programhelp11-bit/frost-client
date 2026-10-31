package dev.frost.core;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.function.Supplier;

/** Reconnecting JSON-lines client for the launcher's loopback bridge. */
public final class FrostLink {
    private final String host;
    private final int port;
    private final Supplier<JsonObject> hello;
    private final FrostState state;
    private volatile OutputStream out;
    private volatile boolean running = true;
    private Thread thread;

    public FrostLink(String host, int port, FrostState state, Supplier<JsonObject> hello) {
        this.host = host;
        this.port = port;
        this.state = state;
        this.hello = hello;
    }

    public void start() {
        thread = new Thread(this::loop, "frost-link");
        thread.setDaemon(true);
        thread.start();
    }

    public void stop() {
        running = false;
        close();
    }

    public boolean connected() {
        return out != null;
    }

    private void loop() {
        while (running) {
            try (Socket s = new Socket()) {
                s.connect(new InetSocketAddress(host, port), 1500);
                s.setTcpNoDelay(true);
                synchronized (this) {
                    out = s.getOutputStream();
                }
                send(hello.get());
                BufferedReader in = new BufferedReader(new InputStreamReader(s.getInputStream(), StandardCharsets.UTF_8));
                String line;
                while (running && (line = in.readLine()) != null) {
                    try {
                        Dispatcher.handle(state, new JsonParser().parse(line).getAsJsonObject());
                    } catch (RuntimeException bad) {
                        // ignore malformed lines, keep the connection
                    }
                }
            } catch (IOException ignored) {
                // launcher not running or connection dropped
            } finally {
                close();
            }
            try {
                Thread.sleep(2000);
            } catch (InterruptedException e) {
                return;
            }
        }
    }

    private synchronized void close() {
        out = null;
    }

    /** Fire-and-forget; silently dropped while disconnected. */
    public synchronized void send(JsonObject msg) {
        OutputStream o = out;
        if (o == null) return;
        try {
            o.write((msg.toString() + "\n").getBytes(StandardCharsets.UTF_8));
            o.flush();
        } catch (IOException e) {
            out = null;
        }
    }
}
