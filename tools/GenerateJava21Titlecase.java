/*
 * Reproduce the Java 21 simple-titlecase ranges used by string_utils.rs.
 * Run with Java 17+; this tool does not call the host Character case APIs.
 *
 * javac tools/GenerateJava21Titlecase.java
 * java -cp tools GenerateJava21Titlecase UnicodeData.txt titlecase-ranges.rsinc
 *
 * Input: OpenJDK jdk-21+35, Unicode 15.0 data, Git blob
 * ea963a7162ce6e913a9f5a98e940b52181ac68dd.
 * https://raw.githubusercontent.com/openjdk/jdk/jdk-21%2B35/src/java.base/share/data/unicodedata/UnicodeData.txt
 * SHA-256 806e9aed65037197f1ec85e12be6e8cd870fc5608b4de0fffd990f689f376a73
 *
 * Unicode mapping data origin and Unicode Data Files and Software License:
 * https://github.com/openjdk/jdk/blob/jdk-21%2B35/src/java.base/share/legal/unicode.md
 * The generated mappings are data facts, not copied Character implementation code.
 * Generator implementation follows this repository's MIT license.
 */

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

public final class GenerateJava21Titlecase {
    private static final String INPUT_SHA256 =
            "806e9aed65037197f1ec85e12be6e8cd870fc5608b4de0fffd990f689f376a73";

    private record Range(int start, int end, int step, int delta) {}

    private static int lookup(List<Range> ranges, int codepoint) {
        int low = 0;
        int high = ranges.size() - 1;
        int best = -1;
        while (low <= high) {
            int middle = (low + high) >>> 1;
            if (ranges.get(middle).start <= codepoint) {
                best = middle;
                low = middle + 1;
            } else {
                high = middle - 1;
            }
        }
        if (best < 0) {
            return codepoint;
        }
        Range range = ranges.get(best);
        return codepoint <= range.end && (codepoint - range.start) % range.step == 0
                ? codepoint + range.delta
                : codepoint;
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected UnicodeData.txt and output .rsinc paths");
        }
        Path input = Path.of(args[0]);
        byte[] bytes = Files.readAllBytes(input);
        String actual = HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
        if (!INPUT_SHA256.equals(actual)) {
            throw new IllegalArgumentException("UnicodeData source SHA-256 differs: " + actual);
        }

        TreeMap<Integer, Integer> pairs = new TreeMap<>();
        for (String row : new String(bytes, StandardCharsets.UTF_8).split("\\R")) {
            if (row.isBlank()) {
                continue;
            }
            String[] fields = row.split(";", -1);
            if (fields.length != 15) {
                throw new IllegalArgumentException("Unexpected UnicodeData column count");
            }
            int codepoint = Integer.parseInt(fields[0], 16);
            // UnicodeData columns are zero-based: simple uppercase is field 12;
            // simple titlecase is field 14. Missing titlecase defaults to uppercase.
            String target = fields[14].isEmpty() ? fields[12] : fields[14];
            if (!target.isEmpty()) {
                int titlecase = Integer.parseInt(target, 16);
                if (titlecase != codepoint) {
                    pairs.put(codepoint, titlecase);
                }
            }
        }

        List<Map.Entry<Integer, Integer>> values = new ArrayList<>(pairs.entrySet());
        List<Range> ranges = new ArrayList<>();
        for (int index = 0; index < values.size();) {
            var first = values.get(index);
            int start = first.getKey();
            int delta = first.getValue() - start;
            int step = 1;
            int last = index;
            if (index + 1 < values.size()) {
                var next = values.get(index + 1);
                int gap = next.getKey() - start;
                if ((gap == 1 || gap == 2) && next.getValue() - next.getKey() == delta) {
                    step = gap;
                    last = index + 1;
                    while (last + 1 < values.size()) {
                        var following = values.get(last + 1);
                        var previous = values.get(last);
                        if (following.getKey() - previous.getKey() != step
                                || following.getValue() - following.getKey() != delta) {
                            break;
                        }
                        last++;
                    }
                }
            }
            ranges.add(new Range(start, values.get(last).getKey(), step, delta));
            index = last + 1;
        }

        // Includes unchanged and unassigned codepoints, so range compression
        // cannot silently introduce a mapping in a gap between changed values.
        for (int codepoint = 0; codepoint <= 0x10ffff; codepoint++) {
            if (lookup(ranges, codepoint) != pairs.getOrDefault(codepoint, codepoint)) {
                throw new AssertionError("Compression changed U+" + Integer.toHexString(codepoint));
            }
        }

        StringBuilder out = new StringBuilder(
                "\n// Generated from OpenJDK jdk-21+35 UnicodeData.txt, blob ea963a7162ce6e913a9f5a98e940b52181ac68dd.\n"
                + "const TITLECASE_RANGES: &[(u32,u32,u32,i32)] = &[\n");
        for (Range range : ranges) {
            out.append(String.format("    (0x%X,0x%X,%d,%d),%n",
                    range.start, range.end, range.step, range.delta));
        }
        out.append("];\n");
        Files.writeString(Path.of(args[1]), out, StandardCharsets.UTF_8);
        System.out.printf("titlecase_pairs=%d ranges=%d all_codepoints_verified=true source=jdk-21+35%n",
                pairs.size(), ranges.size());
    }
}
