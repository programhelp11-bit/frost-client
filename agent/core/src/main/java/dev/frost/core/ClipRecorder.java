package dev.frost.core;

import java.awt.image.BufferedImage;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.io.RandomAccessFile;
import java.text.SimpleDateFormat;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Date;
import java.util.List;
import java.util.concurrent.ArrayBlockingQueue;
import javax.imageio.IIOImage;
import javax.imageio.ImageIO;
import javax.imageio.ImageWriteParam;
import javax.imageio.ImageWriter;
import javax.imageio.stream.MemoryCacheImageOutputStream;

/**
 * Instant-replay recorder: keeps the last N seconds of the game as small JPEG frames in memory and writes them
 * out as a Motion-JPEG .avi (plays in VLC, Windows Media Player, browsers via drag and drop, editors).
 * Pure Java, no ffmpeg and no native code, so it works on Windows and Linux alike.
 */
public final class ClipRecorder {
    private static final class Frame {
        final long at;
        final byte[] jpg;
        final int w, h;

        Frame(long at, byte[] jpg, int w, int h) {
            this.at = at;
            this.jpg = jpg;
            this.w = w;
            this.h = h;
        }
    }

    private final ArrayDeque<Frame> frames = new ArrayDeque<Frame>();
    private final ArrayBlockingQueue<Object[]> queue = new ArrayBlockingQueue<Object[]>(3);
    private volatile Thread worker;
    private long lastWanted;
    public volatile boolean enabled = true;
    public volatile String lastSaved = "";
    public volatile long savedAt;
    public volatile boolean saving;
    private long bytes;

    /** True when it is time for the game to hand over another frame. */
    public boolean wantFrame(Models.Features f) {
        if (!enabled) return false;
        long now = System.nanoTime();
        long gap = 1_000_000_000L / Math.max(5, Math.min(30, f.clip_fps));
        if (now - lastWanted < gap) return false;
        lastWanted = now;
        return queue.remainingCapacity() > 0;
    }

    /** Takes a full-size frame; encoding happens on a background thread. */
    public void submit(BufferedImage img, Models.Features f) {
        if (img == null) return;
        ensureWorker();
        queue.offer(new Object[] {img, Integer.valueOf(f.clip_scale), Integer.valueOf(f.clip_quality), Integer.valueOf(f.clip_seconds)});
    }

    private void ensureWorker() {
        if (worker != null) return;
        synchronized (this) {
            if (worker != null) return;
            Thread t = new Thread(new Runnable() {
                public void run() {
                    while (true) {
                        try {
                            Object[] job = queue.take();
                            encode((BufferedImage) job[0], (Integer) job[1], (Integer) job[2], (Integer) job[3]);
                        } catch (InterruptedException e) {
                            return;
                        } catch (Throwable ignored) {
                            // a bad frame is just skipped
                        }
                    }
                }
            }, "frost-clip");
            t.setDaemon(true);
            worker = t;
            t.start();
        }
    }

    private void encode(BufferedImage src, int scalePct, int quality, int seconds) throws IOException {
        int w = Math.max(64, (src.getWidth() * Math.max(10, Math.min(100, scalePct)) / 100) & ~1);
        int h = Math.max(64, (src.getHeight() * Math.max(10, Math.min(100, scalePct)) / 100) & ~1);
        BufferedImage rgb = new BufferedImage(w, h, BufferedImage.TYPE_INT_RGB);
        java.awt.Graphics2D g = rgb.createGraphics();
        g.setRenderingHint(java.awt.RenderingHints.KEY_INTERPOLATION, java.awt.RenderingHints.VALUE_INTERPOLATION_BILINEAR);
        g.drawImage(src, 0, 0, w, h, null);
        g.dispose();
        ImageWriter wr = ImageIO.getImageWritersByFormatName("jpeg").next();
        ImageWriteParam p = wr.getDefaultWriteParam();
        p.setCompressionMode(ImageWriteParam.MODE_EXPLICIT);
        p.setCompressionQuality(Math.max(0.2f, Math.min(0.95f, quality / 100f)));
        ByteArrayOutputStream out = new ByteArrayOutputStream(w * h / 6);
        MemoryCacheImageOutputStream ios = new MemoryCacheImageOutputStream(out);
        wr.setOutput(ios);
        wr.write(null, new IIOImage(rgb, null, null), p);
        ios.close();
        wr.dispose();
        long now = System.currentTimeMillis();
        synchronized (frames) {
            Frame fr = new Frame(now, out.toByteArray(), w, h);
            frames.addLast(fr);
            bytes += fr.jpg.length;
            long keep = Math.max(5, Math.min(120, seconds)) * 1000L;
            while (!frames.isEmpty() && (now - frames.peekFirst().at > keep || bytes > 400L * 1024 * 1024)) {
                bytes -= frames.pollFirst().jpg.length;
            }
        }
    }

    public int bufferedSeconds() {
        synchronized (frames) {
            if (frames.size() < 2) return 0;
            return (int) ((frames.peekLast().at - frames.peekFirst().at) / 1000L);
        }
    }

    /** Writes the buffered frames to a new file in `dir`; returns the file, or null if there is nothing yet. */
    public File save(File dir) throws IOException {
        List<Frame> copy;
        synchronized (frames) {
            copy = new ArrayList<Frame>(frames);
        }
        if (copy.size() < 2) return null;
        dir.mkdirs();
        File f = new File(dir, "frost-clip-" + new SimpleDateFormat("yyyyMMdd-HHmmss").format(new Date()) + ".avi");
        long span = copy.get(copy.size() - 1).at - copy.get(0).at;
        double fps = Math.max(1.0, Math.min(60.0, copy.size() * 1000.0 / Math.max(1, span)));
        writeAvi(f, copy, fps);
        lastSaved = f.getName();
        savedAt = System.currentTimeMillis();
        return f;
    }

    private static void le32(RandomAccessFile r, long v) throws IOException {
        r.write(new byte[] {(byte) v, (byte) (v >> 8), (byte) (v >> 16), (byte) (v >> 24)});
    }

    private static void le16(RandomAccessFile r, int v) throws IOException {
        r.write(new byte[] {(byte) v, (byte) (v >> 8)});
    }

    private static void tag(RandomAccessFile r, String s) throws IOException {
        r.write(s.getBytes("US-ASCII"));
    }

    /** Minimal OpenDML-free AVI 1.0 writer: one MJPG video stream. */
    static void writeAvi(File file, List<Frame> frames, double fps) throws IOException {
        int w = frames.get(0).w, h = frames.get(0).h;
        RandomAccessFile r = new RandomAccessFile(file, "rw");
        try {
            r.setLength(0);
            long usPerFrame = Math.round(1_000_000.0 / fps);
            tag(r, "RIFF");
            long riffSizePos = r.getFilePointer();
            le32(r, 0);
            tag(r, "AVI ");
            tag(r, "LIST");
            le32(r, 192);                       // hdrl: 4 + (8+56) + (8 + 4+ 8+56 + 8+40)
            tag(r, "hdrl");
            tag(r, "avih");
            le32(r, 56);
            le32(r, usPerFrame);
            le32(r, 0);
            le32(r, 0);
            le32(r, 0x10);                      // has index
            le32(r, frames.size());
            le32(r, 0);
            le32(r, 1);
            le32(r, 0);
            le32(r, w);
            le32(r, h);
            le32(r, 0);
            le32(r, 0);
            le32(r, 0);
            le32(r, 0);
            tag(r, "LIST");
            le32(r, 116);                       // strl: 4 + (8+56) + (8+40)
            tag(r, "strl");
            tag(r, "strh");
            le32(r, 56);
            tag(r, "vids");
            tag(r, "MJPG");
            le32(r, 0);
            le16(r, 0);
            le16(r, 0);
            le32(r, 0);
            le32(r, 1000);                      // scale
            le32(r, Math.round(fps * 1000.0));  // rate
            le32(r, 0);
            le32(r, frames.size());
            le32(r, 1 << 20);
            le32(r, 0xFFFFFFFFL);
            le32(r, 0);
            le16(r, 0);
            le16(r, 0);
            le16(r, w);
            le16(r, h);
            tag(r, "strf");
            le32(r, 40);
            le32(r, 40);
            le32(r, w);
            le32(r, h);
            le16(r, 1);
            le16(r, 24);
            tag(r, "MJPG");
            le32(r, (long) w * h * 3);
            le32(r, 0);
            le32(r, 0);
            le32(r, 0);
            le32(r, 0);
            tag(r, "LIST");
            long moviSizePos = r.getFilePointer();
            le32(r, 0);
            tag(r, "movi");
            long moviStart = r.getFilePointer();
            long[] off = new long[frames.size()];
            for (int i = 0; i < frames.size(); i++) {
                byte[] j = frames.get(i).jpg;
                off[i] = r.getFilePointer() - moviStart + 4;
                tag(r, "00dc");
                le32(r, j.length);
                r.write(j);
                if ((j.length & 1) == 1) r.write(0);
            }
            long moviEnd = r.getFilePointer();
            tag(r, "idx1");
            le32(r, 16L * frames.size());
            for (int i = 0; i < frames.size(); i++) {
                tag(r, "00dc");
                le32(r, 0x10);
                le32(r, off[i]);
                le32(r, frames.get(i).jpg.length);
            }
            long end = r.getFilePointer();
            r.seek(moviSizePos);
            le32(r, moviEnd - moviSizePos - 4);
            r.seek(riffSizePos);
            le32(r, end - 8);
        } finally {
            r.close();
        }
    }
}
