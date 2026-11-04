package dev.frost.mc;

import java.io.InputStream;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

/** Downloads cape PNGs published by the Frost server (version independent; textures are made per module). */
public final class CapeStore {
    private static final HttpClient HTTP = HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(8)).build();
    private static final Map<String, byte[]> BYTES = new ConcurrentHashMap<>();
    private static final Set<String> PENDING = ConcurrentHashMap.newKeySet();
    private static final Map<String, Long> FAILED = new ConcurrentHashMap<>();

    private CapeStore() {}

    /** Cache key for a cape: its content hash when known, otherwise its url. */
    public static String key(String url, String hash) {
        String k = (hash != null && !hash.isEmpty()) ? hash : url;
        return k.toLowerCase().replaceAll("[^a-z0-9._-]", "_");
    }

    /** PNG bytes when downloaded; otherwise starts the download in the background and returns null. */
    public static byte[] bytes(String url, String hash) {
        if (url == null || url.isEmpty()) return null;
        String k = key(url, hash);
        byte[] b = BYTES.get(k);
        if (b != null) return b;
        Long failedAt = FAILED.get(k);
        if (failedAt != null && System.currentTimeMillis() - failedAt < 60_000L) return null;
        if (PENDING.add(k)) {
            Thread t = new Thread(() -> {
                try {
                    HttpRequest req = HttpRequest.newBuilder(URI.create(url)).timeout(Duration.ofSeconds(15)).GET().build();
                    HttpResponse<byte[]> r = HTTP.send(req, HttpResponse.BodyHandlers.ofByteArray());
                    if (r.statusCode() == 200 && r.body().length > 0 && r.body().length <= 512 * 1024) BYTES.put(k, r.body());
                    else FAILED.put(k, System.currentTimeMillis());
                } catch (Exception e) {
                    FAILED.put(k, System.currentTimeMillis());
                } finally {
                    PENDING.remove(k);
                }
            }, "frost-cape-dl");
            t.setDaemon(true);
            t.start();
        }
        return null;
    }
}
