package dev.frost.core;

import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/** BedWars information read from the sidebar scoreboard: event timers and team bed states. */
public final class BwScore {
    /** Seconds since the start of the game at which Hypixel BedWars events happen. */
    private static final String[] EVENT = {"Diamond II", "Emerald II", "Diamond III", "Emerald III", "Bed Gone", "Sudden Death", "Game End"};
    private static final int[] AT = {360, 720, 1080, 1440, 1800, 2400, 3000};

    public static final class Event {
        public final String name;
        public final int secs;

        Event(String name, int secs) {
            this.name = name;
            this.secs = secs;
        }
    }

    public static final class Team {
        public final String letter, name;
        /** 0 = bed alive, 1 = bed destroyed, 2 = eliminated */
        public final int state;
        public final int players;
        public final boolean you;
        public final int color;

        Team(String letter, String name, int state, int players, boolean you, int color) {
            this.letter = letter;
            this.name = name;
            this.state = state;
            this.players = players;
            this.you = you;
            this.color = color;
        }
    }

    public final List<Event> events = new ArrayList<Event>();
    public final List<Team> teams = new ArrayList<Team>();
    public boolean active;

    private static final Pattern NEXT = Pattern.compile("^\\s*([A-Za-z ]+?)\\s+in\\s+(\\d+):(\\d{2})\\s*$");
    private static final Pattern TEAM = Pattern.compile("^\\s*([A-Z])\\s+([A-Za-z]+)\\s*:\\s*(.*)$");

    /** Removes Minecraft colour codes. */
    public static String strip(String s) {
        StringBuilder b = new StringBuilder();
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c == '§' && i + 1 < s.length()) {
                i++;
                continue;
            }
            b.append(c);
        }
        return b.toString();
    }

    private static int colorOf(String raw) {
        int i = raw.indexOf('§');
        if (i < 0 || i + 1 >= raw.length()) return 0xFFFFFFFF;
        switch (Character.toLowerCase(raw.charAt(i + 1))) {
            case 'c': return 0xFFFF5555;
            case '9': return 0xFF5555FF;
            case 'a': return 0xFF55FF55;
            case 'e': return 0xFFFFFF55;
            case 'b': return 0xFF55FFFF;
            case 'f': return 0xFFFFFFFF;
            case 'd': return 0xFFFF55FF;
            case '8': case '7': return 0xFFAAAAAA;
            case '6': return 0xFFFFAA00;
            case '4': return 0xFFAA0000;
            case '1': return 0xFF0000AA;
            case '2': return 0xFF00AA00;
            case '3': return 0xFF00AAAA;
            case '5': return 0xFFAA00AA;
            default: return 0xFFFFFFFF;
        }
    }

    public static BwScore parse(List<String> lines) {
        BwScore r = new BwScore();
        int secsLeft = -1;
        int nextIdx = -1;
        for (String raw : lines) {
            String t = strip(raw).replace(' ', ' ').trim();
            Matcher m = NEXT.matcher(t);
            if (m.matches()) {
                String name = m.group(1).trim();
                for (int i = 0; i < EVENT.length; i++) {
                    if (EVENT[i].equalsIgnoreCase(name)) {
                        nextIdx = i;
                        secsLeft = Integer.parseInt(m.group(2)) * 60 + Integer.parseInt(m.group(3));
                    }
                }
                continue;
            }
            m = TEAM.matcher(t);
            if (m.matches()) {
                String rest = m.group(3).trim();
                boolean you = rest.toUpperCase().contains("YOU");
                int state;
                int players = 0;
                if (rest.contains("✔") || rest.startsWith("✓")) state = 0;
                else if (rest.contains("✘") || rest.contains("✗")) state = 2;
                else {
                    Matcher n = Pattern.compile("(\\d+)").matcher(rest);
                    state = 1;
                    if (n.find()) players = Integer.parseInt(n.group(1));
                }
                r.teams.add(new Team(m.group(1), m.group(2), state, players, you, colorOf(raw)));
            }
        }
        if (nextIdx >= 0) {
            r.active = true;
            int now = AT[nextIdx] - secsLeft;           // game clock in seconds
            for (int i = nextIdx; i < EVENT.length; i++) r.events.add(new Event(EVENT[i], Math.max(0, AT[i] - now)));
        } else if (!r.teams.isEmpty()) {
            r.active = true;
        }
        return r;
    }

    public static String time(int secs) {
        return (secs / 60) + ":" + String.format("%02d", secs % 60);
    }

    /** The next event whose name starts with the prefix ("Diamond", "Emerald"), or null. */
    public Event next(String prefix) {
        for (Event e : events) if (e.name.startsWith(prefix)) return e;
        return null;
    }
}
