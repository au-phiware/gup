#!/usr/bin/env perl
# Copyright (C) 2024 Corin Lawson
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Count non-test lines of code remaining in the frozen old render path
# (RFC-001 migration metric). Strips every #[cfg(test)] item (brace-matched),
# then counts non-blank, non-comment-only lines.
#
# Usage: scripts/old_path_loc.pl            # default old-path file set
#        scripts/old_path_loc.pl FILE...    # explicit files
use strict; use warnings;

if (!@ARGV) {
    my @roots = qw(src/selection.rs src/mark src/shader_function src/shader_pipeline.rs
                   src/chart_builder.rs src/chart_builder src/context.rs src/render.rs);
    @ARGV = grep { -f } map { -d $_ ? split(/\n/, `find $_ -name "*.rs" | sort`) : $_ } @roots;
}

my $total = 0;
for my $path (@ARGV) {
    open(my $fh, '<', $path) or die "cannot open $path: $!";
    my @lines = <$fh>;
    close $fh;

    my @kept;
    my $i = 0;
    my $n = scalar @lines;
    while ($i < $n) {
        my $line = $lines[$i];
        if ($line =~ /^\s*#\[cfg\(test\)\]\s*$/) {
            $i++;
            # skip blank/comment/attribute lines before the item
            while ($i < $n && ($lines[$i] =~ /^\s*#\[/ || $lines[$i] =~ /^\s*\/\// || $lines[$i] =~ /^\s*$/)) {
                $i++;
            }
            my $depth = 0;
            my $started = 0;
            while ($i < $n) {
                my $l = $lines[$i];
                my $opens = () = $l =~ /\{/g;
                my $closes = () = $l =~ /\}/g;
                if ($opens > 0) { $started = 1; }
                $depth += $opens - $closes;
                $i++;
                if ($started && $depth <= 0) { last; }
            }
            next;
        } else {
            push @kept, $line;
            $i++;
        }
    }

    my $count = 0;
    for my $l (@kept) {
        my $s = $l;
        $s =~ s/^\s+|\s+$//g;
        next if $s eq '';
        next if $s =~ m{^//};
        $count++;
    }
    $total += $count;
    printf "%-70s non_test_loc=%d\n", $path, $count;
}
printf "TOTAL\t%d\n", $total;
