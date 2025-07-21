# 2025년 한국 공휴일 데이터
# 주의: 2025년 7월은 공휴일이 없음

holidays_2025 = [
  # 1월
  { date: '2025-01-01', name: '신정', country: 'KR' },
  { date: '2025-01-28', name: '설날 연휴', country: 'KR' },
  { date: '2025-01-29', name: '설날', country: 'KR' },
  { date: '2025-01-30', name: '설날 연휴', country: 'KR' },
  
  # 3월
  { date: '2025-03-01', name: '삼일절', country: 'KR' },
  
  # 5월
  { date: '2025-05-05', name: '어린이날', country: 'KR' },
  { date: '2025-05-06', name: '부처님오신날', country: 'KR' },
  
  # 6월
  { date: '2025-06-06', name: '현충일', country: 'KR' },
  
  # 7월 - 공휴일 없음
  
  # 8월
  { date: '2025-08-15', name: '광복절', country: 'KR' },
  
  # 9월
  { date: '2025-10-05', name: '추석 연휴', country: 'KR' },
  { date: '2025-10-06', name: '추석', country: 'KR' },
  { date: '2025-10-07', name: '추석 연휴', country: 'KR' },
  
  # 10월
  { date: '2025-10-03', name: '개천절', country: 'KR' },
  { date: '2025-10-09', name: '한글날', country: 'KR' },
  
  # 12월
  { date: '2025-12-25', name: '크리스마스', country: 'KR' },
]

holidays_2025.each do |holiday_data|
  Holiday.find_or_create_by(
    date: holiday_data[:date],
    country: holiday_data[:country]
  ) do |holiday|
    holiday.name = holiday_data[:name]
  end
end

puts "2025년 공휴일 데이터 초기화 완료"
puts "- 총 #{holidays_2025.size}개의 공휴일"
puts "- 7월은 공휴일이 없음 (정상)"