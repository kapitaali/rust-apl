// AplUtils.java — Custom Java class for APL FFI demonstration
// Compile: javac AplUtils.java
//
// This class provides static utility methods that can be called from APL
// via the JNI bridge (libapljava.so).

public class AplUtils {
    // Reverse a string
    public static String reverse(String s) {
        return new StringBuilder(s).reverse().toString();
    }

    // Check if string is palindrome
    public static String isPalindrome(String s) {
        String rev = new StringBuilder(s).reverse().toString();
        return String.valueOf(s.equals(rev));
    }

    // String length as string
    public static String length(String s) {
        return String.valueOf(s.length());
    }

    // Substring: input format "text,start,end"
    public static String substring(String input) {
        String[] parts = input.split(",", 3);
        int start = Integer.parseInt(parts[1]);
        int end = Integer.parseInt(parts[2]);
        return parts[0].substring(start, end);
    }

    // Concatenate: input format "a,b"
    public static String concat(String input) {
        String[] parts = input.split(",", 2);
        return parts[0] + parts[1];
    }

    // Replace: input format "text,old,new"
    public static String replace(String input) {
        String[] parts = input.split(",", 3);
        return parts[0].replace(parts[1], parts[2]);
    }

    // To uppercase
    public static String upper(String s) {
        return s.toUpperCase();
    }

    // To lowercase
    public static String lower(String s) {
        return s.toLowerCase();
    }

    // Trim whitespace
    public static String trim(String s) {
        return s.trim();
    }

    // Index of substring: input format "text,substring"
    public static String indexOf(String input) {
        String[] parts = input.split(",", 2);
        return String.valueOf(parts[0].indexOf(parts[1]));
    }

    // Character at index: input format "text,index"
    public static String charAt(String input) {
        String[] parts = input.split(",", 2);
        int idx = Integer.parseInt(parts[1]);
        return String.valueOf(parts[0].charAt(idx));
    }

    // Sort comma-separated values
    public static String sortCsv(String csv) {
        String[] parts = csv.split(",");
        java.util.Arrays.sort(parts);
        return String.join(",", parts);
    }

    // Count words in string
    public static String wordCount(String s) {
        if (s.trim().isEmpty()) return "0";
        return String.valueOf(s.trim().split("\\s+").length);
    }

    // Levenshtein edit distance: input format "a,b"
    public static String levenshtein(String input) {
        String[] parts = input.split(",", 2);
        String a = parts[0], b = parts[1];
        int m = a.length(), n = b.length();
        int[][] dp = new int[m + 1][n + 1];
        for (int i = 0; i <= m; i++) dp[i][0] = i;
        for (int j = 0; j <= n; j++) dp[0][j] = j;
        for (int i = 1; i <= m; i++) {
            for (int j = 1; j <= n; j++) {
                int cost = (a.charAt(i - 1) == b.charAt(j - 1)) ? 0 : 1;
                dp[i][j] = Math.min(Math.min(dp[i - 1][j] + 1, dp[i][j - 1] + 1),
                                   dp[i - 1][j - 1] + cost);
            }
        }
        return String.valueOf(dp[m][n]);
    }

    // Longest common subsequence: input format "a,b"
    public static String lcs(String input) {
        String[] parts = input.split(",", 2);
        String a = parts[0], b = parts[1];
        int m = a.length(), n = b.length();
        int[][] dp = new int[m + 1][n + 1];
        for (int i = 1; i <= m; i++) {
            for (int j = 1; j <= n; j++) {
                if (a.charAt(i - 1) == b.charAt(j - 1))
                    dp[i][j] = dp[i - 1][j - 1] + 1;
                else
                    dp[i][j] = Math.max(dp[i - 1][j], dp[i][j - 1]);
            }
        }
        // Backtrack to find the string
        StringBuilder sb = new StringBuilder();
        int i = m, j = n;
        while (i > 0 && j > 0) {
            if (a.charAt(i - 1) == b.charAt(j - 1)) {
                sb.append(a.charAt(i - 1));
                i--; j--;
            } else if (dp[i - 1][j] > dp[i][j - 1]) {
                i--;
            } else {
                j--;
            }
        }
        return sb.reverse().toString();
    }

    // SHA-256 hash
    public static String sha256(String input) {
        try {
            java.security.MessageDigest md = java.security.MessageDigest.getInstance("SHA-256");
            byte[] hash = md.digest(input.getBytes("UTF-8"));
            StringBuilder sb = new StringBuilder();
            for (byte b : hash) {
                sb.append(String.format("%02x", b));
            }
            return sb.toString();
        } catch (Exception e) {
            return "ERROR: " + e.getMessage();
        }
    }

    // Base64 encode
    public static String base64Encode(String input) {
        return java.util.Base64.getEncoder().encodeToString(input.getBytes());
    }

    // Base64 decode
    public static String base64Decode(String input) {
        return new String(java.util.Base64.getDecoder().decode(input));
    }

    // ROT13 cipher
    public static String rot13(String input) {
        StringBuilder sb = new StringBuilder();
        for (char c : input.toCharArray()) {
            if (c >= 'a' && c <= 'z') {
                sb.append((char) ((c - 'a' + 13) % 26 + 'a'));
            } else if (c >= 'A' && c <= 'Z') {
                sb.append((char) ((c - 'A' + 13) % 26 + 'A'));
            } else {
                sb.append(c);
            }
        }
        return sb.toString();
    }

    // Generate random UUID
    public static String uuid(String ignored) {
        return java.util.UUID.randomUUID().toString();
    }

    // Get system property
    public static String getProperty(String key) {
        return System.getProperty(key);
    }

    // Get environment variable
    public static String getEnv(String name) {
        return System.getenv(name);
    }
}
