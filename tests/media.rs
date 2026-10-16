use pttstella::media::{Kind, classify};

#[test]
fn hover_lease_reauthorizes_only_complete_unchanged_source_without_new_input() {
    use pttstella::{links::Link, media::hover::Lease, navigation::Stamp};
    let stamp = Stamp {
        generation: 1,
        revision: 2,
        input_epoch: 0,
    };
    let link = Link {
        row: 3,
        start: 1,
        end: 24,
        url: String::from("https://example.com/a.png"),
    };
    let mut lease = Lease {
        sequence: 1,
        stamp,
        index: 0,
        link: link.clone(),
    };
    let fresh = Stamp {
        revision: 3,
        ..stamp
    };
    assert!(lease.renew(fresh, true, std::slice::from_ref(&link)));
    assert_eq!(lease.stamp, fresh);
    for (stamp, complete, links) in [
        (
            Stamp {
                input_epoch: 1,
                ..fresh
            },
            true,
            vec![link.clone()],
        ),
        (
            Stamp {
                generation: 2,
                ..fresh
            },
            true,
            vec![link.clone()],
        ),
        (fresh, false, vec![link.clone()]),
        (
            fresh,
            true,
            vec![Link {
                row: 4,
                ..link.clone()
            }],
        ),
        (fresh, true, vec![]),
    ] {
        assert!(!lease.renew(stamp, complete, &links));
    }
}

#[test]
fn direct_media_preserves_query_and_does_not_guess_web_pages() {
    let image = classify("https://example.com/A.JpG?token=a%2Fb#view")
        .unwrap()
        .unwrap();
    assert_eq!(image.kind, Kind::Image);
    assert_eq!(
        image.image_url.as_deref(),
        Some("https://example.com/A.JpG?token=a%2Fb#view")
    );
    assert_eq!(
        classify("https://example.com/movie.webm?x=1")
            .unwrap()
            .unwrap()
            .kind,
        Kind::Video
    );
    for url in [
        "https://imgur.com/abc123",
        "https://example.com/a.gifv",
        "https://example.com/?a=b.png",
        "https://example.com/a.svg",
        "https://example.com/a.mp4.html",
    ] {
        assert_eq!(classify(url).unwrap(), None, "{url}");
    }
}

#[test]
fn youtube_uses_exact_hosts_valid_ids_and_original_playback_url() {
    for url in [
        "https://youtu.be/Abc_12-dEF0?t=12",
        "https://www.youtube.com/watch?v=Abc_12-dEF0&t=12",
        "https://m.youtube.com/shorts/Abc_12-dEF0",
        "https://youtube.com/live/Abc_12-dEF0",
    ] {
        let target = classify(url).unwrap().unwrap();
        assert_eq!(target.kind, Kind::Youtube);
        assert_eq!(target.url, url);
        assert_eq!(
            target.image_url.as_deref(),
            Some("https://i.ytimg.com/vi/Abc_12-dEF0/hqdefault.jpg")
        );
    }
    for url in [
        "https://youtube.com.evil.example/watch?v=Abc_12-dEF0",
        "https://example.com/youtu.be/Abc_12-dEF0",
        "https://youtu.be/short",
        "https://youtu.be/Abc_12-dEF0/extra",
        "https://youtube.com/watch?v=%3Cscript%3E",
    ] {
        assert_eq!(classify(url).unwrap(), None, "{url}");
    }
    for url in [
        "file:///test.png",
        "javascript:alert(1)",
        "https://user:secret@example.com/a.jpg",
    ] {
        assert!(classify(url).is_err());
    }
}
