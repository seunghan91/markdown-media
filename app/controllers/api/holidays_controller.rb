module Api
  class HolidaysController < Api::BaseController
    def index
      year = params[:year].to_i
      month = params[:month].to_i
      
      # 유효한 연도와 월인지 확인
      if year < 1900 || year > 2100 || month < 1 || month > 12
        render json: { error: 'Invalid year or month' }, status: :bad_request
        return
      end
      
      # 여기에 실제 holiday 데이터를 가져오는 로직 추가
      holidays = get_holidays_for_month(year, month)
      
      # 공휴일이 없어도 정상적인 응답 반환 (200 OK)
      render json: { 
        year: year,
        month: month,
        holidays: holidays,
        count: holidays.size
      }, status: :ok
    end
    
    private
    
    def get_holidays_for_month(year, month)
      # 데이터베이스에서 공휴일 조회
      # 공휴일이 없는 경우 빈 배열 반환하는 것이 정상
      # 예: 2025년 7월은 공휴일이 없을 수 있음
      
      country = params[:country] || 'KR'  # 기본값은 한국
      
      holidays = Holiday.for_country(country)
                       .for_month(year, month)
                       .order(:date)
                       .map do |holiday|
        {
          date: holiday.date.to_s,
          name: holiday.name,
          is_substitute: holiday.is_substitute,
          description: holiday.description
        }
      end
      
      holidays
    end
  end
end